use half::f16;
use itertools::Itertools;
use wgsl_parse::{span::Spanned, syntax::*};
use wgsl_types::{
    builtin::{call_binary_op, call_unary_op},
    inst::{Instance, LiteralInstance, RefInstance, VecInstance},
    ty::{Ty, Type},
};

use super::{Context, EvalError, Exec, Flow, ScopeKind, SyntaxUtil};

type E = EvalError;

pub trait Eval {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E>;

    fn eval_value(&self, ctx: &mut Context) -> Result<Instance, E> {
        Ok(self.eval(ctx)?.loaded()?)
    }
}

// this impl exists purely for eval_value()
impl Eval for Instance {
    fn eval(&self, _ctx: &mut Context) -> Result<Instance, E> {
        Ok(self.clone())
    }
}

impl<T: Eval> Eval for Spanned<T> {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        self.node()
            .eval(ctx)
            .inspect_err(|_| ctx.set_err_span_ctx(self.span()))
    }
}

impl Eval for Expression {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        match self {
            Expression::Literal(e) => e.eval(ctx),
            Expression::Parenthesized(e) => e.eval(ctx),
            Expression::NamedComponent(e) => e.eval(ctx),
            Expression::Indexing(e) => e.eval(ctx),
            Expression::Unary(e) => e.eval(ctx),
            Expression::Binary(e) => e.eval(ctx),
            Expression::FunctionCall(e) => e.eval(ctx),
            Expression::TypeOrIdentifier(e) => e.eval(ctx),
        }
    }
}

impl Eval for LiteralExpression {
    fn eval(&self, _ctx: &mut Context) -> Result<Instance, E> {
        match self {
            LiteralExpression::Bool(l) => Ok(LiteralInstance::Bool(*l).into()),
            LiteralExpression::AbstractInt(l) => Ok(LiteralInstance::AbstractInt(*l).into()),
            LiteralExpression::AbstractFloat(l) => Ok(LiteralInstance::AbstractFloat(*l).into()),
            LiteralExpression::I32(l) => Ok(LiteralInstance::I32(*l).into()),
            LiteralExpression::U32(l) => Ok(LiteralInstance::U32(*l).into()),
            LiteralExpression::F32(l) => Ok(LiteralInstance::F32(*l).into()),
            LiteralExpression::F16(l) => {
                let l = f16::from_f32(*l);
                if l.is_infinite() {
                    // this is not supposed to happen.
                    Err(E::Builtin("invalid `f16` literal value (overflow)"))
                } else {
                    Ok(LiteralInstance::F16(l).into()) // TODO: check infinity
                }
            }
            #[cfg(feature = "naga-ext")]
            LiteralExpression::I64(l) => Ok(LiteralInstance::I64(*l).into()),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::U64(l) => Ok(LiteralInstance::U64(*l).into()),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::F64(l) => Ok(LiteralInstance::F64(*l).into()),
        }
    }
}

impl Eval for ParenthesizedExpression {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        self.expression.eval(ctx)
    }
}

impl Eval for NamedComponentExpression {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        fn vec_comp(inst: &Instance, comp: &str, r: Option<&RefInstance>) -> Result<Instance, E> {
            let inst_ty = inst.ty();
            let v = match inst {
                Instance::Vec(v) => v,
                #[cfg(feature = "complex")]
                Instance::Complex(c) => &VecInstance::new(c.iter().cloned().collect_vec()),
                #[cfg(feature = "complex")]
                Instance::Quat(q) => &VecInstance::new(q.iter().cloned().collect_vec()),
                _ => panic!("Non-vectorial type passed to vec_comp"),
            };
            if !check_swizzle(comp) {
                return Err(E::Swizzle(comp.to_string()));
            }
            let indices = comp
                .chars()
                .map(|c| match c {
                    'x' | 'r' => 0usize,
                    'y' | 'g' => 1usize,
                    'z' | 'b' => 2usize,
                    'w' | 'a' => 3usize,
                    _ => unreachable!(), // SAFETY: check_swizzle above checks it.
                })
                .collect_vec();
            if let [i] = indices.as_slice() {
                if let Some(r) = r {
                    Ok(Instance::Ref(r.view_index(*i)?))
                } else {
                    v.get(*i)
                        .cloned()
                        .ok_or_else(|| E::OutOfBounds(*i, inst_ty, v.n()))
                }
            } else {
                let components = indices
                    .into_iter()
                    .map(|i| {
                        v.get(i)
                            .cloned()
                            .ok_or_else(|| E::OutOfBounds(i, inst_ty.clone(), v.n()))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(VecInstance::new(components).into())
            }
        }

        fn inst_comp(base: Instance, comp: &str) -> Result<Instance, E> {
            match &base {
                Instance::Struct(s) => {
                    let val = s
                        .member(comp)
                        .ok_or_else(|| E::Component(s.ty(), comp.to_string()))?;
                    Ok(val.clone())
                }
                v @ Instance::Vec(_) => vec_comp(v, comp, None),
                #[cfg(feature = "complex")]
                c @ Instance::Complex(_) => vec_comp(c, comp, None),
                #[cfg(feature = "complex")]
                q @ Instance::Quat(_) => vec_comp(q, comp, None),
                Instance::Ref(r) => match &*r.read()? {
                    Instance::Struct(_) => Ok(r.view_member(comp.to_string())?.into()),
                    v @ Instance::Vec(_) => vec_comp(v, comp, Some(r)),
                    #[cfg(feature = "complex")]
                    c @ Instance::Complex(_) => vec_comp(c, comp, Some(r)),
                    #[cfg(feature = "complex")]
                    q @ Instance::Quat(_) => vec_comp(q, comp, Some(r)),
                    _ => Err(E::Component(base.ty(), comp.to_string())),
                },
                _ => Err(E::Component(base.ty(), comp.to_string())),
            }
        }

        let base = self.base.eval(ctx)?;
        inst_comp(base, &self.component.name())
    }
}

impl Eval for IndexingExpression {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        fn index_inst(base: &Instance, index: usize) -> Result<Instance, E> {
            match base {
                Instance::Vec(v) => v
                    .get(index)
                    .cloned()
                    .ok_or_else(|| E::OutOfBounds(index, v.ty(), v.n())),
                Instance::Mat(m) => m
                    .col(index)
                    .cloned()
                    .ok_or_else(|| E::OutOfBounds(index, m.ty(), m.c())),
                Instance::Array(a) => a
                    .get(index)
                    .cloned()
                    .ok_or_else(|| E::OutOfBounds(index, a.ty(), a.n())),
                Instance::Ref(r) => Ok(r.view_index(index)?.into()),
                _ => Err(E::NotIndexable(base.ty())),
            }
        }

        let base = self.base.eval(ctx)?;
        let index = self.index.eval_value(ctx)?;
        let index = match index {
            Instance::Literal(LiteralInstance::AbstractInt(i)) => Ok(i as usize),
            Instance::Literal(LiteralInstance::I32(i)) => Ok(i as usize),
            Instance::Literal(LiteralInstance::U32(i)) => Ok(i as usize),
            _ => Err(E::Index(index.ty())),
        }?;

        index_inst(&base, index)
    }
}

impl Eval for UnaryExpression {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        // TODO: operators should work with non-loaded instances
        // (performs the load rule). Is it the case already?
        if self.operator == UnaryOperator::AddressOf {
            let operand = self.operand.eval(ctx)?;
            operand.op_ref()
        } else {
            let operand = self.operand.eval_value(ctx)?;
            call_unary_op(self.operator, &operand)
        }
        .map_err(Into::into)
    }
}

impl Eval for BinaryExpression {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        let lhs = self.left.eval_value(ctx)?;

        if self.operator == BinaryOperator::ShortCircuitOr {
            match lhs {
                Instance::Literal(LiteralInstance::Bool(true)) => Ok(lhs),
                Instance::Literal(LiteralInstance::Bool(false)) => {
                    let rhs = self.right.eval_value(ctx)?;
                    match rhs {
                        Instance::Literal(LiteralInstance::Bool(true)) => Ok(rhs),
                        Instance::Literal(LiteralInstance::Bool(false)) => Ok(rhs),
                        _ => Err(E::Binary(self.operator, lhs.ty(), rhs.ty())),
                    }
                }
                _ => Err(E::Binary(self.operator, lhs.ty(), Type::Bool)),
            }
        } else if self.operator == BinaryOperator::ShortCircuitAnd {
            match lhs {
                Instance::Literal(LiteralInstance::Bool(true)) => {
                    let rhs = self.right.eval_value(ctx)?;
                    match rhs {
                        Instance::Literal(LiteralInstance::Bool(true)) => Ok(rhs),
                        Instance::Literal(LiteralInstance::Bool(false)) => Ok(rhs),
                        _ => Err(E::Binary(self.operator, lhs.ty(), rhs.ty())),
                    }
                }
                Instance::Literal(LiteralInstance::Bool(false)) => Ok(lhs),
                _ => Err(E::Binary(self.operator, lhs.ty(), Type::Bool)),
            }
        } else {
            let rhs = self.right.eval_value(ctx)?;
            call_binary_op(self.operator, &lhs, &rhs, ctx.stage).map_err(Into::into)
        }
    }
}

impl Eval for FunctionCall {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        let flow = self.exec(ctx)?;
        match flow {
            Flow::Next | Flow::Break | Flow::Continue | Flow::Return(None) => {
                Err(E::Void(self.ty.ident.to_string()))
            }
            Flow::Return(Some(inst)) => Ok(inst),
        }
    }
}

impl Eval for TypeExpression {
    /// See also [`ty_eval_ty`][super::ty::ty_eval_ty]. This implementation evaluates TypeExpression that are
    /// identifiers, not types.
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        if self.template_args.is_some() {
            Err(E::UnexpectedTemplate(self.ident.to_string()))
        } else if let Some(inst) = ctx.scope.get(&self.ident.name()) {
            if matches!(inst, Instance::Deferred(_)) {
                Err(E::NotAccessible(self.ident.to_string(), ctx.stage))
            } else {
                Ok(inst.clone())
            }
        } else {
            if ctx.kind == ScopeKind::Module {
                // there is hoisting at module-scope. We may refer to a later declaration.
                if let Some(decl) = ctx.source.decl(&self.ident.name()) {
                    decl.exec(ctx)?;
                    if let Some(inst) = ctx.scope.get(&self.ident.name()) {
                        return if matches!(inst, Instance::Deferred(_)) {
                            Err(E::NotAccessible(self.ident.to_string(), ctx.stage))
                        } else {
                            Ok(inst.clone())
                        };
                    }
                }
            }
            Err(E::UnknownDecl(self.ident.to_string()))
        }
    }
}

impl Eval for TemplateArg {
    fn eval(&self, ctx: &mut Context) -> Result<Instance, E> {
        self.expression.eval(ctx)
    }
}

pub(crate) fn check_swizzle(swizzle: &str) -> bool {
    // reference: https://www.w3.org/TR/WGSL/#swizzle-names
    (1..=4).contains(&swizzle.len())
        && (swizzle.chars().all(|c| "xyzw".contains(c))
            || swizzle.chars().all(|c| "rgba".contains(c)))
}
