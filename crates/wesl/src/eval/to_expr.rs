use wgsl_types::{
    inst::{ArrayInstance, Instance, LiteralInstance, MatInstance, StructInstance, VecInstance},
    ty::{TextureType, Ty, Type},
};

use super::SyntaxUtil;

use crate::{
    eval::{Context, EvalError},
    idents::{BuiltinIdent, builtin_ident},
};
use wgsl_parse::{span::Spanned, syntax::*};

type E = EvalError;

/// Convert an instance to an Expression.
pub trait ToExpr {
    fn to_expr(&self, ctx: &Context) -> Result<Expression, E>;
}

impl ToExpr for Instance {
    fn to_expr(&self, ctx: &Context) -> Result<Expression, E> {
        match self {
            Instance::Literal(inst) => inst.to_expr(ctx),
            Instance::Struct(inst) => inst.to_expr(ctx),
            Instance::Array(inst) => inst.to_expr(ctx),
            Instance::Vec(inst) => inst.to_expr(ctx),
            Instance::Mat(inst) => inst.to_expr(ctx),
            Instance::Ptr(_) | Instance::Ref(_) | Instance::Atomic(_) | Instance::Opaque(_) => {
                Err(E::NotConstructible(self.ty()))
            }
        }
    }
}

impl ToExpr for LiteralInstance {
    fn to_expr(&self, _ctx: &Context) -> Result<Expression, E> {
        Ok(match self {
            LiteralInstance::Bool(lit) => LiteralExpression::Bool(*lit),
            LiteralInstance::AbstractInt(lit) => LiteralExpression::AbstractInt(*lit),
            LiteralInstance::AbstractFloat(lit) => LiteralExpression::AbstractFloat(*lit),
            LiteralInstance::I32(lit) => LiteralExpression::I32(*lit),
            LiteralInstance::U32(lit) => LiteralExpression::U32(*lit),
            LiteralInstance::F32(lit) => LiteralExpression::F32(*lit),
            LiteralInstance::F16(lit) => LiteralExpression::F16(lit.to_f32()),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(lit) => LiteralExpression::I64(*lit),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(lit) => LiteralExpression::U64(*lit),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(lit) => LiteralExpression::F64(*lit),
        }
        .into())
    }
}

impl ToExpr for StructInstance {
    fn to_expr(&self, ctx: &Context) -> Result<Expression, E> {
        let decl = ctx
            .source
            .decl_struct(&self.ty.name)
            .expect("struct declaration not found");
        Ok(Expression::FunctionCall(FunctionCall {
            ty: TypeExpression::new(decl.ident.clone()),
            arguments: decl
                .members
                .iter()
                .map(|m| {
                    self.member(&m.ident.name())
                        .expect("struct member not found")
                        .to_expr(ctx)
                        .map(Spanned::from)
                })
                .collect::<Result<Vec<_>, _>>()?,
        }))
    }
}

impl ToExpr for ArrayInstance {
    fn to_expr(&self, ctx: &Context) -> Result<Expression, E> {
        Ok(Expression::FunctionCall(FunctionCall {
            ty: TypeExpression::new(builtin_ident("array").unwrap().clone()),
            arguments: self
                .iter()
                .map(|c| c.to_expr(ctx).map(Spanned::from))
                .collect::<Result<Vec<_>, _>>()?,
        }))
    }
}

impl ToExpr for VecInstance {
    fn to_expr(&self, ctx: &Context) -> Result<Expression, E> {
        Ok(Expression::FunctionCall(FunctionCall {
            ty: TypeExpression::new(self.ty().builtin_ident().unwrap().clone()),
            arguments: self
                .iter()
                .map(|c| c.to_expr(ctx).map(Spanned::from))
                .collect::<Result<Vec<_>, _>>()?,
        }))
    }
}

impl ToExpr for MatInstance {
    fn to_expr(&self, ctx: &Context) -> Result<Expression, E> {
        Ok(Expression::FunctionCall(FunctionCall {
            ty: TypeExpression::new(self.ty().builtin_ident().unwrap().clone()),
            arguments: self
                .iter() // could also use iter_cols here to output matCxR(VecR(), ...)
                .map(|c| c.to_expr(ctx).map(Spanned::from))
                .collect::<Result<Vec<_>, _>>()?,
        }))
    }
}

impl ToExpr for Type {
    fn to_expr(&self, ctx: &Context) -> Result<Expression, E> {
        let ident = self.builtin_ident().cloned();
        match self {
            Type::Bool => Ok(TypeExpression::new(ident.unwrap())),
            Type::AbstractInt => Err(E::NotConstructible(Type::AbstractInt)),
            Type::AbstractFloat => Err(E::NotConstructible(Type::AbstractFloat)),
            Type::I32 => Ok(TypeExpression::new(ident.unwrap())),
            Type::U32 => Ok(TypeExpression::new(ident.unwrap())),
            Type::F32 => Ok(TypeExpression::new(ident.unwrap())),
            Type::F16 => Ok(TypeExpression::new(ident.unwrap())),
            #[cfg(feature = "naga-ext")]
            Type::I64 => Ok(TypeExpression::new(ident.unwrap())),
            #[cfg(feature = "naga-ext")]
            Type::U64 => Ok(TypeExpression::new(ident.unwrap())),
            #[cfg(feature = "naga-ext")]
            Type::F64 => Ok(TypeExpression::new(ident.unwrap())),
            Type::Struct(s) => {
                let decl = ctx
                    .source
                    .decl_struct(&s.name)
                    .expect("struct declaration not found");
                Ok(TypeExpression::new(decl.ident.clone()))
            }
            Type::Array(inner_ty, Some(n)) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = Some(vec![
                    TemplateArg {
                        expression: inner_ty.to_expr(ctx)?.into(),
                    },
                    TemplateArg {
                        expression: Expression::Literal(LiteralExpression::AbstractInt(*n as i64))
                            .into(),
                    },
                ]);
                Ok(ty)
            }
            Type::Array(inner_ty, None) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = Some(vec![TemplateArg {
                    expression: inner_ty.to_expr(ctx)?.into(),
                }]);
                Ok(ty)
            }
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(inner_ty, Some(n)) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = Some(vec![
                    TemplateArg {
                        expression: inner_ty.to_expr(ctx)?.into(),
                    },
                    TemplateArg {
                        expression: Expression::Literal(LiteralExpression::AbstractInt(*n as i64))
                            .into(),
                    },
                ]);
                Ok(ty)
            }
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(inner_ty, None) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = Some(vec![TemplateArg {
                    expression: inner_ty.to_expr(ctx)?.into(),
                }]);
                Ok(ty)
            }
            Type::Vec(_, inner_ty) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = Some(vec![TemplateArg {
                    expression: inner_ty.to_expr(ctx)?.into(),
                }]);
                Ok(ty)
            }
            Type::Mat(_, _, inner_ty) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = Some(vec![TemplateArg {
                    expression: inner_ty.to_expr(ctx)?.into(),
                }]);
                Ok(ty)
            }
            Type::Atomic(inner_ty) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = Some(vec![TemplateArg {
                    expression: inner_ty.to_expr(ctx)?.into(),
                }]);
                Ok(ty)
            }
            Type::Ptr(a_s, inner_ty, a_m) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                let t1 = TemplateArg {
                    expression: a_s.to_expr(ctx)?.into(),
                };
                let t2 = TemplateArg {
                    expression: inner_ty.to_expr(ctx)?.into(),
                };
                let args = if let AddressSpace::Storage = a_s {
                    let t3 = TemplateArg {
                        expression: a_m.to_expr(ctx)?.into(),
                    };
                    vec![t1, t2, t3]
                } else {
                    vec![t1, t2]
                };
                ty.template_args = Some(args);
                Ok(ty)
            }
            Type::Ref(_, _, _) => Err(E::NotConstructible(self.ty())),
            Type::Texture(tex) => {
                let mut ty = TypeExpression::new(ident.unwrap());
                ty.template_args = match tex {
                    TextureType::Sampled1D(sampled)
                    | TextureType::Sampled2D(sampled)
                    | TextureType::Sampled2DArray(sampled)
                    | TextureType::Sampled3D(sampled)
                    | TextureType::SampledCube(sampled)
                    | TextureType::SampledCubeArray(sampled)
                    | TextureType::Multisampled2D(sampled) => Some(vec![TemplateArg {
                        expression: Expression::TypeOrIdentifier(TypeExpression::new(
                            builtin_ident(&sampled.to_string()).unwrap().clone(),
                        ))
                        .into(),
                    }]),
                    TextureType::DepthMultisampled2D => None,
                    TextureType::External => None,
                    TextureType::Storage1D(texel, access)
                    | TextureType::Storage2D(texel, access)
                    | TextureType::Storage2DArray(texel, access)
                    | TextureType::Storage3D(texel, access) => Some(vec![
                        TemplateArg {
                            expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                builtin_ident(&texel.to_string()).unwrap().clone(),
                            ))
                            .into(),
                        },
                        TemplateArg {
                            expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                builtin_ident(&access.to_string()).unwrap().clone(),
                            ))
                            .into(),
                        },
                    ]),
                    TextureType::Depth2D => None,
                    TextureType::Depth2DArray => None,
                    TextureType::DepthCube => None,
                    TextureType::DepthCubeArray => None,
                    #[cfg(feature = "naga-ext")]
                    TextureType::Sampled1DArray(sampled)
                    | TextureType::Multisampled2DArray(sampled) => Some(vec![TemplateArg {
                        expression: Expression::TypeOrIdentifier(TypeExpression::new(
                            builtin_ident(&sampled.to_string()).unwrap().clone(),
                        ))
                        .into(),
                    }]),
                    #[cfg(feature = "naga-ext")]
                    TextureType::Storage1DArray(texel, access) => Some(vec![
                        TemplateArg {
                            expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                builtin_ident(&texel.to_string()).unwrap().clone(),
                            ))
                            .into(),
                        },
                        TemplateArg {
                            expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                builtin_ident(&access.to_string()).unwrap().clone(),
                            ))
                            .into(),
                        },
                    ]),
                };
                Ok(ty)
            }
            Type::Sampler(_) => Ok(TypeExpression::new(ident.unwrap())),
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(flags) | Type::AccelerationStructure(flags) => Ok(TypeExpression {
                path: None,
                ident: ident.unwrap(),
                template_args: (*flags == Some(AccelerationStructureFlags::VertexReturn)).then(
                    || {
                        vec![TemplateArg {
                            expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                builtin_ident("vertex_return").unwrap().clone(),
                            ))
                            .into(),
                        }]
                    },
                ),
            }),
        }
        .map(Expression::from)
    }
}

impl ToExpr for AddressSpace {
    fn to_expr(&self, _ctx: &Context) -> Result<Expression, E> {
        Ok(TypeExpression::new(self.builtin_ident().unwrap().clone()).into())
    }
}

impl ToExpr for AccessMode {
    fn to_expr(&self, _ctx: &Context) -> Result<Expression, E> {
        Ok(TypeExpression::new(self.builtin_ident().unwrap().clone()).into())
    }
}
