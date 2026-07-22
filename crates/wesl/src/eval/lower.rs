use std::iter::zip;
use wgsl_parse::{SyntaxNode, span::Spanned, syntax::*};
use wgsl_types::ty::{Ty, Type};

use crate::eval::{ATTR_INTRINSIC, Context, Eval, EvalError, Exec, ty_eval_ty};

use super::{
    CompoundScope, EXPR_FALSE, EXPR_TRUE, EvalTy, Instance, ShaderStage, SyntaxUtil,
    to_expr::ToExpr, with_scope,
};

type E = EvalError;

fn make_explicit_call(call: &mut FunctionCall, ctx: &mut Context) -> Result<(), E> {
    let decl = ctx.source.decl_function(&call.ty.ident.name());
    if let Some(decl) = decl {
        if decl.body.contains_attribute(&ATTR_INTRINSIC) {
            // we only do explicit conversions on user-defined functions,
            // because built-in functions have overloads for abstract types.
            return Ok(());
        }

        for (arg, param) in zip(&mut call.arguments, &decl.parameters) {
            let arg_ty = arg.eval_ty(ctx)?.loaded();
            let param_ty = ty_eval_ty(&param.ty, ctx)?;
            if arg_ty != param_ty {
                if arg_ty.is_convertible_to(&param_ty) {
                    let ty = param_ty.to_expr(ctx)?.unwrap_type_or_identifier();
                    *arg.node_mut() = Expression::FunctionCall(FunctionCall {
                        ty,
                        arguments: vec![arg.clone()],
                    })
                } else {
                    return Err(E::ParamType(param_ty, arg_ty));
                }
            }
        }
    }
    Ok(())
}

/// Replace automatic conversions with explicit calls to the target type's constructor.
fn make_explicit_return(stmt: &mut ReturnStatement, ctx: &mut Context) -> Result<(), E> {
    // HACK: we use ctx.err_decl to keep track of the current function, which is not supposed to be
    // set at this stage in normal conditions.
    let decl = ctx
        .source
        .decl_function(ctx.err_decl.as_ref().unwrap())
        .unwrap();

    match (&mut stmt.expression, &decl.return_type) {
        (None, None) => Ok(()),
        (None, Some(ret_expr)) => {
            let ret_ty = ty_eval_ty(ret_expr, ctx)?;
            Err(E::NoReturn(decl.ident.to_string(), ret_ty))
        }
        (Some(expr), None) => {
            let expr_ty = expr.eval_ty(ctx)?;
            Err(E::UnexpectedReturn(decl.ident.to_string(), expr_ty))
        }
        (Some(expr), Some(ret_expr)) => {
            let ret_ty = ty_eval_ty(ret_expr, ctx)?;
            let expr_ty = expr.eval_ty(ctx)?.loaded();

            if expr_ty == ret_ty || expr_ty.is_array() && ret_ty.is_array() {
                // array<> does not have a constructor that takes another array.
                return Ok(());
            }

            if expr_ty.is_convertible_to(&ret_ty) {
                let ty = ret_ty.to_expr(ctx)?.unwrap_type_or_identifier();
                *expr.node_mut() = Expression::FunctionCall(FunctionCall {
                    ty,
                    arguments: vec![expr.clone()],
                });
                Ok(())
            } else {
                Err(E::ReturnType(expr_ty, decl.ident.to_string(), ret_ty))
            }
        }
    }
}

pub trait Lower {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E>;
}

impl<T: Lower> Lower for Option<T> {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        if let Some(x) = self {
            x.lower(ctx)?;
        }
        Ok(())
    }
}

impl<T: Lower> Lower for Spanned<T> {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.node_mut()
            .lower(ctx)
            .inspect_err(|_| ctx.set_err_span_ctx(self.span()))?;
        Ok(())
    }
}

impl Lower for Expression {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        match self.eval_value(ctx) {
            Ok(inst) => *self = inst.to_expr(ctx)?,
            // These are supposed to be the only acceptable errors when evaluating valid code.
            Err(E::Todo(_) | E::NotAccessible(_, ShaderStage::Const) | E::NotConst(_)) => {
                ctx.err_span = None;
                match self {
                    Expression::Literal(_) => (),
                    Expression::Parenthesized(expr) => expr.expression.lower(ctx)?,
                    Expression::NamedComponent(expr) => expr.base.lower(ctx)?,
                    Expression::Indexing(expr) => {
                        expr.base.lower(ctx)?;
                        expr.index.lower(ctx)?;
                    }
                    Expression::Unary(expr) => expr.operand.lower(ctx)?,
                    Expression::Binary(expr) => {
                        expr.left.lower(ctx)?;
                        expr.right.lower(ctx)?;
                    }
                    Expression::FunctionCall(expr) => expr.lower(ctx)?,
                    Expression::TypeOrIdentifier(_) => {
                        if let Ok(expr) = self.eval_value(ctx).and_then(|inst| inst.to_expr(ctx)) {
                            *self = expr;
                        }
                    }
                }
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }
}

impl Lower for FunctionCall {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.ty = ctx.source.resolve_ty(&self.ty).clone();

        // replace automatic conversions with explicit calls to the target type's constructor
        make_explicit_call(self, ctx)?;

        for arg in &mut self.arguments {
            arg.lower(ctx)?;
        }

        Ok(())
    }
}

impl Lower for TemplateArgs {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        if let Some(tplts) = self {
            for tplt in tplts {
                tplt.expression.lower(ctx)?;
            }
        }
        Ok(())
    }
}

impl Lower for TypeExpression {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        // types must be const-expressions
        let expr = ty_eval_ty(self, ctx)?
            .to_expr(ctx)?
            .unwrap_type_or_identifier();
        *self = expr;
        Ok(())
    }
}

impl Lower for Attributes {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        for attr in self {
            match attr.node_mut() {
                Attribute::Align(expr)
                | Attribute::Binding(expr)
                | Attribute::BlendSrc(expr)
                | Attribute::Group(expr)
                | Attribute::Id(expr)
                | Attribute::Location(expr)
                | Attribute::Size(expr) => {
                    expr.lower(ctx)?;
                }
                Attribute::WorkgroupSize(attr) => {
                    attr.x.lower(ctx)?;
                    attr.y.lower(ctx)?;
                    attr.z.lower(ctx)?;
                }
                Attribute::Custom(_) => {
                    // we ignore unknown attributes for now. We don't know how they are implemented.
                }
                _ => (),
            }
        }
        Ok(())
    }
}

impl Lower for Declaration {
    // this one is called only for function-scope declarations.
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.attributes.lower(ctx)?;
        self.initializer.lower(ctx)?;

        // TODO: this is copy-pasted 3 times now.
        let ty = match (&self.ty, &self.initializer) {
            (None, None) => return Err(E::UntypedDecl),
            (None, Some(init)) => {
                let ty = init.eval_ty(ctx)?.loaded();
                if self.kind.is_const() {
                    ty // only const declarations can be of abstract type.
                } else {
                    ty.concretize()
                }
            }
            (Some(ty), _) => ty_eval_ty(ty, ctx)?,
        };

        if ty.is_concrete() {
            self.ty = Some(ty.to_expr(ctx)?.unwrap_type_or_identifier());
        }

        let inst_ty = if let DeclarationKind::Var(as_am) = self.kind {
            let (a_s, a_m) = match as_am {
                Some((a_s, Some(a_m))) => (a_s, a_m),
                Some((a_s, None)) => (a_s, a_s.default_access_mode()),
                None => (AddressSpace::Function, AccessMode::ReadWrite),
            };
            Type::Ref(a_s, Box::new(ty), a_m)
        } else {
            ty
        };

        if ctx
            .scope
            .add(self.ident.to_string(), Instance::Deferred(inst_ty))
        {
            Ok(())
        } else {
            Err(E::DuplicateDecl(self.ident.to_string()))
        }
    }
}

impl Lower for Struct {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        for m in &mut self.members {
            m.attributes.lower(ctx)?;
            m.ty.lower(ctx)?;
        }
        Ok(())
    }
}

impl Lower for Function {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.attributes.lower(ctx)?;
        for p in &mut self.parameters {
            p.attributes.lower(ctx)?;
            p.ty.lower(ctx)?;
        }
        self.return_attributes.lower(ctx)?;
        self.return_type.lower(ctx)?;

        // HACK: we need to keep track of the declaration name to lower the return statements and
        // we repurpose `ctx.err_ctx` for that.
        ctx.err_decl = Some(self.ident.to_string());

        with_scope!(ctx, {
            for p in &self.parameters {
                let inst = Instance::Deferred(ty_eval_ty(&p.ty, ctx)?);
                if !ctx.scope.add(p.ident.to_string(), inst) {
                    return Err(E::DuplicateDecl(p.ident.to_string()));
                }
            }
            compound_lower(&mut self.body, ctx, CompoundScope::Transparent)?;
            Ok(())
        })?;

        // ideally we would do this first, but it would prevent validation errors from triggering
        // in `compound_lower`.
        if self.contains_attribute(&Attribute::Const) && self.return_type.is_none() {
            self.body.statements.clear();
        }
        Ok(())
    }
}

impl Lower for Statement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        match self {
            Statement::Void => (),
            Statement::Compound(stmt) => {
                stmt.lower(ctx)?;
                if stmt.statements.is_empty() {
                    *self = Statement::Void;
                } else if let [stmt] = stmt.statements.as_slice() {
                    *self = stmt.node().clone();
                }
            }
            Statement::Assignment(stmt) => stmt.lower(ctx)?,
            Statement::Increment(stmt) => stmt.lower(ctx)?,
            Statement::Decrement(stmt) => stmt.lower(ctx)?,
            Statement::If(stmt) => {
                stmt.lower(ctx)?;

                // remove clauses evaluating to false
                stmt.else_if_clauses
                    .retain(|clause| *clause.expression != EXPR_FALSE);

                // remove subsequent clauses after a true
                if let Some(i) = stmt
                    .else_if_clauses
                    .iter()
                    .position(|clause| *clause.expression == EXPR_TRUE)
                {
                    stmt.else_if_clauses.resize_with(i + 1, || unreachable!());
                    stmt.else_clause = None;
                }

                macro_rules! assign_clause {
                    ($stmt:ident, $body:expr) => {
                        if $body.statements.is_empty() {
                            *$stmt = Statement::Void;
                        } else if let [s1] = $body.statements.as_slice() {
                            *$stmt = s1.node().clone();
                        } else {
                            *$stmt = Statement::Compound($body.clone())
                        }
                    };
                }

                // if the first clause is true, replace the statement with the body
                if *stmt.if_clause.expression == EXPR_TRUE {
                    assign_clause!(self, stmt.if_clause.body);
                }
                // if the clause is false, replace with the body of the `else` branch or the first
                // `else if`. if neither exist, replace the entire statement with void.
                else if *stmt.if_clause.expression == EXPR_FALSE {
                    if let Some(clause) = stmt.else_if_clauses.first() {
                        if *clause.expression == EXPR_TRUE {
                            assign_clause!(self, clause.body);
                        }
                    } else if let Some(clause) = &stmt.else_clause {
                        assign_clause!(self, clause.body);
                    } else {
                        *self = Statement::Void;
                    }
                }
            }
            Statement::Switch(stmt) => stmt.lower(ctx)?,
            Statement::Loop(stmt) => stmt.lower(ctx)?,
            Statement::For(stmt) => {
                stmt.lower(ctx)?;
                if stmt
                    .condition
                    .as_ref()
                    .is_some_and(|cond| **cond == EXPR_FALSE)
                {
                    *self = Statement::Void;
                }
            }
            Statement::While(stmt) => {
                stmt.lower(ctx)?;
                if *stmt.condition == EXPR_FALSE {
                    *self = Statement::Void;
                }
            }
            Statement::Break(_) => (),
            Statement::Continue(_) => (),
            Statement::Return(stmt) => stmt.lower(ctx)?,
            Statement::Discard(_) => (),
            Statement::FunctionCall(stmt) => {
                let decl = ctx.source.decl_function(&stmt.call.ty.ident.name());
                if let Some(decl) = decl {
                    if decl.contains_attribute(&Attribute::Const)
                        && !decl.contains_attribute(&Attribute::MustUse)
                    {
                        *self = Statement::Void; // a const function has no side-effects
                    } else {
                        stmt.lower(ctx)?
                    }
                } else {
                    stmt.lower(ctx)?
                }
            }
            Statement::ConstAssert(stmt) => stmt.exec(ctx).map(|_| ())?,
            Statement::Declaration(stmt) => {
                if stmt.kind == DeclarationKind::Const {
                    // eval and add it to the scope
                    stmt.exec(ctx)?;
                    *self = Statement::Void;
                } else {
                    stmt.lower(ctx)?;
                }
            }
        }
        Ok(())
    }
}

fn compound_lower(
    stmt: &mut CompoundStatement,
    ctx: &mut Context,
    scoping: CompoundScope,
) -> Result<(), E> {
    stmt.attributes.lower(ctx)?;
    with_scope!(ctx, scoping, {
        for stmt in &mut stmt.statements {
            stmt.lower(ctx)?;
        }
        Result::<(), E>::Ok(())
    })?;
    stmt.statements.retain(|stmt| match stmt.node() {
        Statement::Void => false,
        Statement::Compound(_) => true,
        Statement::Assignment(_) => true,
        Statement::Increment(_) => true,
        Statement::Decrement(_) => true,
        Statement::If(_) => true,
        Statement::Switch(_) => true,
        Statement::Loop(_) => true,
        Statement::For(_) => true,
        Statement::While(_) => true,
        Statement::Break(_) => true,
        Statement::Continue(_) => true,
        Statement::Return(_) => true,
        Statement::Discard(_) => true,
        Statement::FunctionCall(_) => true,
        Statement::ConstAssert(_) => false,
        Statement::Declaration(_) => true,
    });
    Ok(())
}

impl Lower for CompoundStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        compound_lower(self, ctx, CompoundScope::Regular)
    }
}

impl Lower for AssignmentStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        let is_phony = matches!(self.lhs.node(), Expression::TypeOrIdentifier(TypeExpression { path: None, ident, template_args: None }) if *ident.name() == "_");
        if !is_phony {
            self.lhs.lower(ctx)?;
        }
        self.rhs.lower(ctx)?;
        Ok(())
    }
}

impl Lower for IncrementStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.expression.lower(ctx)?;
        Ok(())
    }
}

impl Lower for DecrementStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.expression.lower(ctx)?;
        Ok(())
    }
}

impl Lower for IfStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.attributes.lower(ctx)?;
        self.if_clause.expression.lower(ctx)?;
        self.if_clause.body.lower(ctx)?;
        for clause in &mut self.else_if_clauses {
            clause.expression.lower(ctx)?;
            clause.body.lower(ctx)?;
        }
        if let Some(clause) = &mut self.else_clause {
            clause.body.lower(ctx)?;
        }
        Ok(())
    }
}

impl Lower for SwitchStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.attributes.lower(ctx)?;
        self.expression.lower(ctx)?;
        self.body_attributes.lower(ctx)?;
        for clause in &mut self.clauses {
            for sel in &mut clause.case_selectors {
                if let CaseSelector::Expression(expr) = sel {
                    expr.lower(ctx)?;
                }
            }
            clause.body.lower(ctx)?;
        }
        Ok(())
    }
}

impl Lower for LoopStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        with_scope!(ctx, {
            compound_lower(&mut self.body, ctx, CompoundScope::Leaking)?;

            if let Some(cont) = &mut self.continuing {
                with_scope!(ctx, {
                    compound_lower(&mut cont.body, ctx, CompoundScope::Leaking)?;
                    if let Some(break_if) = &mut cont.break_if {
                        break_if.expression.lower(ctx)?;
                    }
                    Result::<_, E>::Ok(())
                })?;
            }

            Ok(())
        })
    }
}

impl Lower for ForStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        // the initializer is in the same scope as the body.
        // https://github.com/gpuweb/gpuweb/issues/5024
        with_scope!(ctx, {
            self.initializer.lower(ctx)?;
            self.condition.lower(ctx)?;
            self.update.lower(ctx)?;
            compound_lower(&mut self.body, ctx, CompoundScope::Transparent)?;
            Ok(())
        })
    }
}

impl Lower for WhileStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.condition.lower(ctx)?;
        self.body.lower(ctx)?;
        Ok(())
    }
}

impl Lower for ReturnStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        // replace automatic conversions with explicit calls to the target type's constructor
        // we call this before lower, because the explicit call can then be lowered.
        make_explicit_return(self, ctx)?;

        self.expression.lower(ctx)?;
        Ok(())
    }
}

impl Lower for FunctionCallStatement {
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        self.call.lower(ctx)
    }
}

impl Lower for TranslationUnit {
    /// expects that `exec` was called on the TU before.
    fn lower(&mut self, ctx: &mut Context) -> Result<(), E> {
        for decl in &mut self.global_declarations {
            match decl.node_mut() {
                GlobalDeclaration::Void => Ok(()),
                GlobalDeclaration::Declaration(decl) => {
                    decl.attributes.lower(ctx)?;
                    decl.initializer.lower(ctx)?;

                    let inst = ctx
                        .scope
                        .get(&decl.ident.name())
                        .expect("module-scope declaration not present in scope");
                    let ty = inst.ty().loaded();

                    if ty.is_concrete() {
                        decl.ty = Some(ty.to_expr(ctx)?.unwrap_type_or_identifier());
                    }

                    Ok(())
                }
                GlobalDeclaration::TypeAlias(_) => Ok(()), // no need to lower it will be eliminated
                GlobalDeclaration::Struct(decl) => decl.lower(ctx),
                GlobalDeclaration::Function(decl) => decl.lower(ctx),
                GlobalDeclaration::ConstAssert(_) => Ok(()), // handled by TranslationUnit::exec()
            }
            .inspect_err(|_| {
                if let Some(ident) = decl.ident() {
                    ctx.set_err_decl_ctx(ident.to_string())
                }
            })?;
        }
        self.global_declarations.retain(|decl| match decl.node() {
            GlobalDeclaration::Void => false,
            GlobalDeclaration::Declaration(decl) => decl.kind != DeclarationKind::Const,
            GlobalDeclaration::TypeAlias(_) => false,
            GlobalDeclaration::Struct(_) => true,
            GlobalDeclaration::Function(_) => true,
            GlobalDeclaration::ConstAssert(_) => false,
        });
        Ok(())
    }
}
