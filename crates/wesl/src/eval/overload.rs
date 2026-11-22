//! Function overload resolution.
//!
//! WESL allows declaring multiple functions with the same name in a module, as long as
//! their parameter types differ. Calls are resolved to an overload with the
//! [WGSL overload resolution algorithm](https://www.w3.org/TR/WGSL/#overload-resolution-section).

use std::{collections::HashMap, iter::zip};

use wgsl_parse::{SyntaxNode, span::Spanned, syntax::*};
use wgsl_types::{conv::conversion_rank, inst::Instance, ty::Type};

use super::{
    CompoundScope, Context, EvalError, EvalTy, Exec, ScopeKind, SyntaxUtil, ty_eval_ty, with_scope,
};

type E = EvalError;

/// Overload sets: maps the shared identifier of an overload set to the signature and the
/// new identifier of each overload, in declaration order.
pub type OverloadSets = HashMap<Ident, Vec<(Vec<Type>, Ident)>>;

/// Evaluate the parameter types of a function.
pub fn fn_signature(decl: &Function, ctx: &mut Context) -> Result<Vec<Type>, E> {
    decl.parameters
        .iter()
        .map(|p| ty_eval_ty(&p.ty, ctx))
        .collect()
}

/// Select the overload matching the argument types.
///
/// Implements the [WGSL overload resolution algorithm](https://www.w3.org/TR/WGSL/#overload-resolution-section).
/// Returns the index of the selected signature.
pub fn select_overload(name: &str, signatures: &[Vec<Type>], args: &[Type]) -> Result<usize, E> {
    // candidates are the overloads for which a feasible automatic conversion exists for
    // each argument. We record the conversion rank of each argument.
    let candidates = signatures
        .iter()
        .enumerate()
        .filter(|(_, sig)| sig.len() == args.len())
        .filter_map(|(i, sig)| {
            zip(args, sig)
                .map(|(arg, param)| conversion_rank(arg, param))
                .collect::<Option<Vec<_>>>()
                .map(|ranks| (i, ranks))
        })
        .collect::<Vec<_>>();

    // Note: the WGSL step eliminating candidates with abstract parameters after conversion
    // does not apply, because user-declared parameters always have a concrete type.

    // C1 is preferred over C2 if all its conversion ranks are lower or equal, and at least
    // one is strictly lower.
    let preferred = |r1: &[u32], r2: &[u32]| {
        zip(r1, r2).all(|(a, b)| a <= b) && zip(r1, r2).any(|(a, b)| a < b)
    };

    let mut best = candidates
        .iter()
        .filter(|(i, r1)| candidates.iter().all(|(j, r2)| i == j || preferred(r1, r2)));

    match (best.next(), best.next()) {
        (Some((i, _)), None) => Ok(*i),
        _ if candidates.is_empty() => Err(E::NoOverload(name.to_string(), args.to_vec())),
        _ => Err(E::AmbiguousOverload(name.to_string(), args.to_vec())),
    }
}

/// Select the overload of a function called with the given argument types.
///
/// Returns `None` if the function is not overloaded.
pub(crate) fn select_function<'s>(
    name: &str,
    args: &[Type],
    ctx: &mut Context<'s>,
) -> Result<Option<&'s Function>, E> {
    let source: &'s TranslationUnit = ctx.source;
    let candidates = source.decl_functions(name);
    if candidates.len() < 2 {
        return Ok(None);
    }
    let signatures = candidates
        .iter()
        .map(|decl| fn_signature(decl, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    let i = select_overload(name, &signatures, args)?;
    Ok(Some(candidates[i]))
}

/// Whether a function name refers to an overloaded function.
pub(crate) fn is_overloaded(name: &str, ctx: &Context) -> bool {
    ctx.source.decl_functions(name).len() > 1
}

/// Populate the context scope with module-scope declarations.
///
/// Declarations that cannot be evaluated are added with an opaque instance of their type
/// when possible, or ignored otherwise.
pub(crate) fn init_module_scope(ctx: &mut Context) {
    fn add_decl(decl: &Declaration, ctx: &mut Context) {
        if ctx.scope.contains(&decl.ident.name()) || decl.exec(ctx).is_ok() {
            return;
        }
        if let Ok(ty) = decl_ty(decl, ctx) {
            ctx.scope
                .add(decl.ident.to_string(), Instance::Opaque(inst_ty(decl, ty)));
        }
    }

    fn add_decls<'a>(
        decls: impl IntoIterator<Item = &'a GlobalDeclarationNode>,
        ctx: &mut Context,
    ) {
        for decl in decls {
            match decl.node() {
                GlobalDeclaration::Declaration(decl) => add_decl(decl, ctx),
                GlobalDeclaration::Compound(decl) => add_decls(&decl.body, ctx),
                _ => (),
            }
            ctx.err_decl = None;
            ctx.err_span = None;
        }
    }

    let kind = ctx.kind;
    ctx.kind = ScopeKind::Module;
    let source: &TranslationUnit = ctx.source;
    add_decls(&source.global_declarations, ctx);
    ctx.kind = kind;
}

fn decl_ty(decl: &Declaration, ctx: &mut Context) -> Result<Type, E> {
    match (&decl.ty, &decl.initializer) {
        (None, None) => Err(E::UntypedDecl),
        (None, Some(init)) => {
            let ty = init.eval_ty(ctx)?.loaded();
            if decl.kind.is_const() {
                Ok(ty) // only const declarations can be of abstract type.
            } else {
                Ok(ty.concretize())
            }
        }
        (Some(ty), _) => ty_eval_ty(ty, ctx),
    }
}

fn inst_ty(decl: &Declaration, ty: Type) -> Type {
    if let DeclarationKind::Var(as_am) = decl.kind {
        let (a_s, a_m) = match as_am {
            Some((a_s, Some(a_m))) => (a_s, a_m),
            Some((a_s, None)) => (a_s, a_s.default_access_mode()),
            None => (AddressSpace::Function, AccessMode::ReadWrite),
        };
        Type::Ref(a_s, Box::new(ty), a_m)
    } else {
        ty
    }
}

/// Rewrite calls to overloaded functions to point at the selected overload identifier.
///
/// The context source must be a copy of the module before any rewrite, and the module scope
/// must be populated (see [`init_module_scope`]). Types are always evaluated before the
/// rewrite of a node, so that the context source can still resolve them.
pub(crate) fn resolve_calls(
    module: &mut TranslationUnit,
    ctx: &mut Context,
    sets: &OverloadSets,
) -> Result<(), E> {
    fn resolve_decls<'a>(
        decls: impl IntoIterator<Item = &'a mut GlobalDeclarationNode>,
        ctx: &mut Context,
        sets: &OverloadSets,
    ) -> Result<(), E> {
        for decl in decls {
            let ident = decl.ident();
            match decl.node_mut() {
                GlobalDeclaration::Void => Ok(()),
                GlobalDeclaration::Declaration(decl) => {
                    let kind = ctx.kind;
                    ctx.kind = ScopeKind::Module;
                    let res = (|| {
                        decl.attributes.resolve(ctx, sets)?;
                        decl.ty.resolve(ctx, sets)?;
                        decl.initializer.resolve(ctx, sets)
                    })();
                    ctx.kind = kind;
                    res
                }
                GlobalDeclaration::TypeAlias(decl) => decl.ty.resolve(ctx, sets),
                GlobalDeclaration::Struct(decl) => decl.members.iter_mut().try_for_each(|m| {
                    m.attributes.resolve(ctx, sets)?;
                    m.ty.resolve(ctx, sets)
                }),
                GlobalDeclaration::Function(decl) => resolve_fn(decl, ctx, sets),
                GlobalDeclaration::ConstAssert(decl) => decl.expression.resolve(ctx, sets),
                GlobalDeclaration::Compound(decl) => resolve_decls(&mut decl.body, ctx, sets),
            }
            .inspect_err(|_| {
                if let Some(ident) = ident {
                    ctx.set_err_decl_ctx(ident.to_string())
                }
            })?;
        }
        Ok(())
    }

    resolve_decls(&mut module.global_declarations, ctx, sets)
}

trait Resolve {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E>;
}

impl<T: Resolve> Resolve for Option<T> {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        if let Some(x) = self {
            x.resolve(ctx, sets)?;
        }
        Ok(())
    }
}

impl<T: Resolve> Resolve for Spanned<T> {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        let span = self.span();
        self.node_mut()
            .resolve(ctx, sets)
            .inspect_err(|_| ctx.set_err_span_ctx(span))
    }
}

impl Resolve for Attributes {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        for attr in self {
            match attr.node_mut() {
                Attribute::Align(expr)
                | Attribute::Binding(expr)
                | Attribute::BlendSrc(expr)
                | Attribute::Group(expr)
                | Attribute::Id(expr)
                | Attribute::Location(expr)
                | Attribute::Size(expr) => expr.resolve(ctx, sets)?,
                Attribute::WorkgroupSize(attr) => {
                    attr.x.resolve(ctx, sets)?;
                    attr.y.resolve(ctx, sets)?;
                    attr.z.resolve(ctx, sets)?;
                }
                Attribute::Custom(attr) => {
                    for arg in attr.arguments.iter_mut().flatten() {
                        arg.resolve(ctx, sets)?;
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}

impl Resolve for TypeExpression {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        for arg in self.template_args.iter_mut().flatten() {
            arg.expression.resolve(ctx, sets)?;
        }
        Ok(())
    }
}

impl Resolve for Expression {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        match self {
            Expression::Literal(_) => Ok(()),
            Expression::Parenthesized(expr) => expr.expression.resolve(ctx, sets),
            Expression::NamedComponent(expr) => expr.base.resolve(ctx, sets),
            Expression::Indexing(expr) => {
                expr.base.resolve(ctx, sets)?;
                expr.index.resolve(ctx, sets)
            }
            Expression::Unary(expr) => expr.operand.resolve(ctx, sets),
            Expression::Binary(expr) => {
                expr.left.resolve(ctx, sets)?;
                expr.right.resolve(ctx, sets)
            }
            Expression::FunctionCall(expr) => expr.resolve(ctx, sets),
            Expression::TypeOrIdentifier(expr) => expr.resolve(ctx, sets),
        }
    }
}

impl Resolve for FunctionCall {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        if let Some(overloads) = sets.get(&self.ty.ident) {
            // the argument types must be evaluated before the arguments are rewritten.
            let args = self
                .arguments
                .iter()
                .map(|arg| arg.eval_ty(ctx).map(|ty| ty.loaded()))
                .collect::<Result<Vec<_>, _>>()?;
            let signatures = overloads
                .iter()
                .map(|(sig, _)| sig.clone())
                .collect::<Vec<_>>();
            let i = select_overload(&self.ty.ident.name(), &signatures, &args)?;
            self.ty.ident = overloads[i].1.clone();
        }
        self.ty.resolve(ctx, sets)?;
        for arg in &mut self.arguments {
            arg.resolve(ctx, sets)?;
        }
        Ok(())
    }
}

fn resolve_fn(decl: &mut Function, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
    // the parameter types must be evaluated before they are rewritten.
    let params = decl
        .parameters
        .iter()
        .map(|p| ty_eval_ty(&p.ty, ctx))
        .collect::<Result<Vec<_>, _>>()?;

    decl.attributes.resolve(ctx, sets)?;
    for p in &mut decl.parameters {
        p.attributes.resolve(ctx, sets)?;
        p.ty.resolve(ctx, sets)?;
    }
    decl.return_attributes.resolve(ctx, sets)?;
    decl.return_type.resolve(ctx, sets)?;

    let kind = ctx.kind;
    ctx.kind = ScopeKind::Function;
    let res = with_scope!(ctx, {
        for (p, ty) in zip(&decl.parameters, params) {
            if !ctx.scope.add(p.ident.to_string(), Instance::Opaque(ty)) {
                return Err(E::DuplicateDecl(p.ident.to_string()));
            }
        }
        resolve_compound(&mut decl.body, ctx, sets, CompoundScope::Transparent)
    });
    ctx.kind = kind;
    res
}

fn resolve_compound(
    stmt: &mut CompoundStatement,
    ctx: &mut Context,
    sets: &OverloadSets,
    scoping: CompoundScope,
) -> Result<(), E> {
    stmt.attributes.resolve(ctx, sets)?;
    with_scope!(ctx, scoping, {
        for stmt in &mut stmt.statements {
            stmt.resolve(ctx, sets)?;
        }
        Ok(())
    })
}

impl Resolve for CompoundStatement {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        resolve_compound(self, ctx, sets, CompoundScope::Regular)
    }
}

impl Resolve for Declaration {
    // this one is called only for function-scope declarations.
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        // the declaration type must be evaluated before the initializer is rewritten.
        let added = self.kind.is_const() && {
            // const declarations are evaluated, because they can be used in templates.
            let res = self.exec(ctx).is_ok();
            ctx.err_decl = None;
            ctx.err_span = None;
            res
        };
        if !added {
            let ty = inst_ty(self, decl_ty(self, ctx)?);
            if !ctx.scope.add(self.ident.to_string(), Instance::Opaque(ty)) {
                return Err(E::DuplicateDecl(self.ident.to_string()));
            }
        }

        self.attributes.resolve(ctx, sets)?;
        self.ty.resolve(ctx, sets)?;
        self.initializer.resolve(ctx, sets)
    }
}

impl Resolve for Statement {
    fn resolve(&mut self, ctx: &mut Context, sets: &OverloadSets) -> Result<(), E> {
        match self {
            Statement::Void
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Discard(_) => Ok(()),
            Statement::Compound(stmt) => stmt.resolve(ctx, sets),
            Statement::Assignment(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.lhs.resolve(ctx, sets)?;
                stmt.rhs.resolve(ctx, sets)
            }
            Statement::Increment(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.expression.resolve(ctx, sets)
            }
            Statement::Decrement(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.expression.resolve(ctx, sets)
            }
            Statement::If(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.if_clause.expression.resolve(ctx, sets)?;
                stmt.if_clause.body.resolve(ctx, sets)?;
                for clause in &mut stmt.else_if_clauses {
                    clause.attributes.resolve(ctx, sets)?;
                    clause.expression.resolve(ctx, sets)?;
                    clause.body.resolve(ctx, sets)?;
                }
                if let Some(clause) = &mut stmt.else_clause {
                    clause.attributes.resolve(ctx, sets)?;
                    clause.body.resolve(ctx, sets)?;
                }
                Ok(())
            }
            Statement::Switch(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.expression.resolve(ctx, sets)?;
                stmt.body_attributes.resolve(ctx, sets)?;
                for clause in &mut stmt.clauses {
                    clause.attributes.resolve(ctx, sets)?;
                    for sel in &mut clause.case_selectors {
                        if let CaseSelector::Expression(expr) = sel {
                            expr.resolve(ctx, sets)?;
                        }
                    }
                    clause.body.resolve(ctx, sets)?;
                }
                Ok(())
            }
            Statement::Loop(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                with_scope!(ctx, {
                    resolve_compound(&mut stmt.body, ctx, sets, CompoundScope::Leaking)?;
                    if let Some(cont) = &mut stmt.continuing {
                        cont.attributes.resolve(ctx, sets)?;
                        with_scope!(ctx, {
                            resolve_compound(&mut cont.body, ctx, sets, CompoundScope::Leaking)?;
                            if let Some(break_if) = &mut cont.break_if {
                                break_if.attributes.resolve(ctx, sets)?;
                                break_if.expression.resolve(ctx, sets)?;
                            }
                            Ok::<(), E>(())
                        })?;
                    }
                    Ok(())
                })
            }
            Statement::For(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                // the initializer is in the same scope as the body.
                with_scope!(ctx, {
                    stmt.initializer.resolve(ctx, sets)?;
                    stmt.condition.resolve(ctx, sets)?;
                    stmt.update.resolve(ctx, sets)?;
                    resolve_compound(&mut stmt.body, ctx, sets, CompoundScope::Transparent)
                })
            }
            Statement::While(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.condition.resolve(ctx, sets)?;
                stmt.body.resolve(ctx, sets)
            }
            Statement::Return(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.expression.resolve(ctx, sets)
            }
            Statement::FunctionCall(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.call.resolve(ctx, sets)
            }
            Statement::ConstAssert(stmt) => {
                stmt.attributes.resolve(ctx, sets)?;
                stmt.expression.resolve(ctx, sets)
            }
            Statement::Declaration(stmt) => stmt.resolve(ctx, sets),
        }
    }
}
