use super::{Scope, SyntaxUtil};
use itertools::Itertools;
use wgsl_parse::{SyntaxNode, span::Spanned, syntax::*};
use wgsl_types::builtin::is_ctor;

macro_rules! with_scope {
    ($scope:expr, $body:tt) => {{
        $scope.push();
        #[allow(clippy::redundant_closure_call)]
        let body = (|| $body)();
        $scope.pop();
        body
    }};
}

pub(crate) fn mark_functions_const(wesl: &mut TranslationUnit) {
    let mut locals = Locals::new();
    let is_const = wesl
        .global_declarations
        .iter()
        .map(|decl| {
            if let GlobalDeclaration::Function(decl) = decl.node() {
                locals.add(decl.ident.to_string(), true); // we suppose the function is const.
                let is_const = decl.is_const(wesl, &mut locals);
                if !is_const {
                    *locals.local_get_mut(&decl.ident.name()).unwrap() = false;
                }
                return is_const;
            }
            false
        })
        .collect_vec();
    for (decl, is_const) in wesl.global_declarations.iter_mut().zip(is_const) {
        if let GlobalDeclaration::Function(decl) = decl.node_mut()
            && is_const
            && !decl.contains_attribute(&Attribute::Const)
        {
            decl.attributes.push(Attribute::Const.into())
        }
    }
}

impl IsConst for Function {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.contains_attribute(&Attribute::Const)
            || with_scope!(locals, {
                self.attributes.is_const(wesl, locals)
                    && self.parameters.is_const(wesl, locals)
                    && self.return_attributes.is_const(wesl, locals)
                    && self.return_type.is_const(wesl, locals)
                    && self.body.attributes.is_const(wesl, locals)
                    && self.body.statements.is_const(wesl, locals)
            })
    }
}

type Locals = Scope<bool>;

trait IsConst {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool;
}

impl<T: IsConst> IsConst for Option<T> {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.as_ref()
            .map(|x| x.is_const(wesl, locals))
            .unwrap_or(true)
    }
}

impl<T: IsConst> IsConst for Spanned<T> {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.node().is_const(wesl, locals)
    }
}

impl<T: IsConst> IsConst for Vec<T> {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.iter().all(|x| x.is_const(wesl, locals))
    }
}

impl IsConst for Struct {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.members
            .iter()
            .all(|m| m.attributes.is_const(wesl, locals) && m.ty.is_const(wesl, locals))
    }
}

impl IsConst for Attribute {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        match self {
            Attribute::Align(expr) => expr.is_const(wesl, locals),
            Attribute::Binding(expr) => expr.is_const(wesl, locals),
            Attribute::BlendSrc(expr) => expr.is_const(wesl, locals),
            Attribute::Builtin(_) => false, // attr on entrypoints params (never const)
            Attribute::Const => true,
            Attribute::Diagnostic(_) => true,
            Attribute::Group(_) => false,
            Attribute::Id(_) => false, // attr on overridables (never const)
            Attribute::Interpolate(_) => false, // attr on entrypoints params (never const)
            Attribute::Invariant => false, // attr on entrypoints ret type (never const)
            Attribute::Location(_) => false, // attr on entrypoints params (never const)
            Attribute::MustUse => true,
            Attribute::Size(expr) => expr.is_const(wesl, locals),
            Attribute::WorkgroupSize(_) => false, // attr on entrypoint function (never const)
            Attribute::Vertex => false,           // attr on entrypoint function (never const)
            Attribute::Fragment => false,         // attr on entrypoint function (never const)
            Attribute::Compute => false,          // attr on entrypoint function (never const)
            Attribute::Publish => true,           // imports are const
            Attribute::If(_) => true,             // if attributes are translate-time (always const)
            Attribute::Elif(_) => true,           // if attributes are translate-time (always const)
            Attribute::Else => true,              // if attributes are translate-time (always const)
            #[cfg(feature = "generics")]
            Attribute::Type(_) => todo!(),
            #[cfg(feature = "naga-ext")]
            Attribute::EarlyDepthTest(_) => true,
            Attribute::Custom(attr) => attr.arguments.is_const(wesl, locals),
        }
    }
}

impl IsConst for FormalParameter {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.attributes.is_const(wesl, locals) && self.ty.is_const(wesl, locals) && {
            locals.add(self.ident.to_string(), true);
            true
        }
    }
}

impl IsConst for TypeExpression {
    // keep in mind a TypeExpression can be either a type or a reference.
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        let ty = wesl.resolve_ty(self);
        if let Some(args) = &self.template_args {
            // template args = refer to a built-in type generator.
            // having all template args be const (aka constructible) is sufficient for valid code.
            args.iter().all(|arg| arg.expression.is_const(wesl, locals))
        } else {
            // constructible types with no template are scalars and constructible structs.
            // is the TypeExpression is a reference, only global consts or locals can be const.
            match ty.ident.name().as_str() {
                "bool" | "i32" | "u32" | "f32" | "f16" => true,
                name => {
                    locals.contains(name)
                        || wesl
                            .decl_struct(name)
                            .is_some_and(|decl| decl.is_const(wesl, locals))
                        || wesl
                            .decl_decl(name)
                            .is_some_and(|decl| decl.kind == DeclarationKind::Const)
                }
            }
        }
    }
}

impl IsConst for Statement {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        match self {
            Statement::Void => true,
            Statement::Compound(stmt) => stmt.is_const(wesl, locals),
            Statement::Assignment(stmt) => {
                stmt.lhs.is_const(wesl, locals) && stmt.rhs.is_const(wesl, locals)
            }
            Statement::Increment(stmt) => stmt.expression.is_const(wesl, locals),
            Statement::Decrement(stmt) => stmt.expression.is_const(wesl, locals),
            Statement::If(stmt) => {
                stmt.attributes.is_const(wesl, locals)
                    && stmt.if_clause.expression.is_const(wesl, locals)
                    && stmt.if_clause.body.is_const(wesl, locals)
                    && stmt.else_if_clauses.iter().all(|clause| {
                        clause.expression.is_const(wesl, locals)
                            && clause.body.is_const(wesl, locals)
                    })
                    && stmt
                        .else_clause
                        .as_ref()
                        .map(|clause| clause.body.is_const(wesl, locals))
                        .unwrap_or(true)
            }
            Statement::Switch(stmt) => {
                stmt.attributes.is_const(wesl, locals)
                    && stmt.expression.is_const(wesl, locals)
                    && stmt.body_attributes.is_const(wesl, locals)
                    && stmt.clauses.iter().all(|clause| {
                        clause.case_selectors.iter().all(|sel| match sel {
                            CaseSelector::Default => true,
                            CaseSelector::Expression(expr) => expr.is_const(wesl, locals),
                        }) && clause.body.is_const(wesl, locals)
                    })
            }
            Statement::Loop(stmt) => {
                stmt.attributes.is_const(wesl, locals)
                    && stmt.body.is_const(wesl, locals)
                    && stmt.continuing.is_const(wesl, locals)
            }
            Statement::For(stmt) => {
                stmt.attributes.is_const(wesl, locals)
                    && stmt.initializer.is_const(wesl, locals)
                    && stmt.condition.is_const(wesl, locals)
                    && stmt.update.is_const(wesl, locals)
                    && stmt.body.is_const(wesl, locals)
            }
            Statement::While(stmt) => {
                stmt.attributes.is_const(wesl, locals)
                    && stmt.condition.is_const(wesl, locals)
                    && stmt.body.is_const(wesl, locals)
            }
            Statement::Break(_) => true,
            Statement::Continue(_) => true,
            Statement::Return(stmt) => stmt.expression.is_const(wesl, locals),
            Statement::Discard(_) => false, // only in entrypoints, never const
            Statement::FunctionCall(stmt) => stmt.call.is_const(wesl, locals),
            Statement::ConstAssert(_) => true,
            Statement::Declaration(stmt) => {
                stmt.attributes.is_const(wesl, locals)
                    && stmt.ty.is_const(wesl, locals)
                    && stmt.initializer.is_const(wesl, locals)
                    && {
                        locals.add(stmt.ident.to_string(), true);
                        true
                    }
            }
        }
    }
}

impl IsConst for ContinuingStatement {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.body.is_const(wesl, locals)
            && self
                .break_if
                .as_ref()
                .map(|stmt| stmt.expression.is_const(wesl, locals))
                .unwrap_or(true)
    }
}

impl IsConst for CompoundStatement {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.attributes.is_const(wesl, locals) && {
            locals.push();
            let res = self.statements.is_const(wesl, locals);
            locals.pop();
            res
        }
    }
}

impl IsConst for Expression {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        match self {
            Expression::Literal(_) => true,
            Expression::Parenthesized(expr) => expr.expression.is_const(wesl, locals),
            Expression::NamedComponent(expr) => expr.base.is_const(wesl, locals),
            Expression::Indexing(expr) => {
                expr.base.is_const(wesl, locals) && expr.index.is_const(wesl, locals)
            }
            Expression::Unary(expr) => expr.operand.is_const(wesl, locals),
            Expression::Binary(expr) => {
                expr.left.is_const(wesl, locals) && expr.right.is_const(wesl, locals)
            }
            Expression::FunctionCall(call) => call.is_const(wesl, locals),
            Expression::TypeOrIdentifier(ty) => ty.is_const(wesl, locals),
        }
    }
}

impl IsConst for FunctionCall {
    fn is_const(&self, wesl: &TranslationUnit, locals: &mut Locals) -> bool {
        self.arguments.iter().all(|arg| arg.is_const(wesl, locals)) && {
            let ty = wesl.resolve_ty(&self.ty);
            let fn_name = ty.ident.name();

            if let Some(is_const) = locals.get(&fn_name) {
                *is_const
            } else if let Some(decl) = wesl.decl_struct(&fn_name) {
                decl.is_const(wesl, locals)
            } else if let Some(decl) = wesl.decl_function(&fn_name) {
                // TODO: this is not optimal as it will be recomputed for the same functions.
                decl.is_const(wesl, locals)
            } else {
                is_ctor(&fn_name)
            }
        }
    }
}
