// this file is unfinished.

use std::ops::Deref;

use derive_more::derive::{Deref, DerefMut};
use itertools::Itertools;
use naga::{Arena, SpecialTypes, UniqueArena};
use thiserror::Error;

use wgsl_parse::{SyntaxNode, span::Span, syntax::*};

use crate::{
    EvalError, Exec,
    eval::{self, Context, EvalAttrs, EvalTy, ShaderStage, SyntaxUtil, Type},
};

#[derive(Clone, Debug, Error)]
pub enum NagaError {
    #[error("{0}")]
    Eval(EvalError),
    #[error("f16 are not supported by naga")]
    F16NotSupported,
    #[error("TODO in wgsl to naga conversion")]
    TODO,
    #[error("expected an AbstractInt, wgsl to naga does not support all expressions here")]
    ExpectedAbstractInt,
    #[error("wgsl to naga does not support {0}")]
    Unsupported(&'static str),
    #[error("uninitialized const-declaration")]
    UninitConst,
    #[error("let-declarations are not permitted at the module scope")]
    LetInMod,
}

type E = NagaError;
type ExprArena = Arena<naga::Expression>;
type TypeArena = UniqueArena<naga::Type>;

#[derive(Deref, DerefMut)]
struct ToNaga<T>(T);

impl ToNaga<&Type> {
    pub fn to_naga(&self, ctx: &mut Context, arena: &mut TypeArena) -> Result<naga::TypeInner, E> {
        match self {
            Type::Bool => Ok(naga::TypeInner::Scalar(naga::Scalar::BOOL)),
            Type::AbstractInt => Ok(naga::TypeInner::Scalar(naga::Scalar::ABSTRACT_INT)),
            Type::AbstractFloat => Ok(naga::TypeInner::Scalar(naga::Scalar::ABSTRACT_FLOAT)),
            Type::I32 => Ok(naga::TypeInner::Scalar(naga::Scalar::I32)),
            Type::U32 => Ok(naga::TypeInner::Scalar(naga::Scalar::U32)),
            Type::F32 => Ok(naga::TypeInner::Scalar(naga::Scalar::F32)),
            Type::F16 => Err(E::F16NotSupported),
            Type::Struct(name) => {
                let decl = ctx.source.decl_struct(name).unwrap();
                ToNaga(decl).to_naga(ctx, arena)
            }
            Type::Array(n, ty) => {
                let base = arena.insert(ToNaga(&ty).to_naga(ctx, arena)?, naga::Span::UNDEFINED);
                let size = match n {
                    Some(n) => naga::ArraySize::Constant(n),
                    None => naga::ArraySize::Dynamic,
                };
                naga::TypeInner::Array {
                    base,
                    size,
                    stride: (),
                }
            }
            Type::Vec(_, _) => todo!(),
            Type::Mat(_, _, _) => todo!(),
            Type::Atomic(_) => todo!(),
            Type::Ptr(address_space, _) => todo!(),
            Type::Texture(texture_type) => todo!(),
            Type::Sampler(sampler_type) => todo!(),
            Type::Void => todo!(),
        }
    }
}

impl ToNaga<&TypeExpression> {
    pub fn to_naga(&self, ctx: &mut Context) -> Result<naga::Type, E> {
        Ok(naga::Type {
            name: Some(self.ident.to_string()),
            inner: match self.ident.name().as_str() {
                "f32" => naga::TypeInner::Scalar(naga::Scalar {
                    kind: naga::ScalarKind::Float,
                    width: 4,
                }),
                _ => return Err(E::TODO),
            },
        })
    }
}

impl ToNaga<&LiteralExpression> {
    pub fn to_naga(&self) -> Result<naga::Literal, E> {
        Ok(match self.deref() {
            LiteralExpression::Bool(x) => naga::Literal::Bool(*x),
            LiteralExpression::AbstractInt(x) => naga::Literal::AbstractInt(*x),
            LiteralExpression::AbstractFloat(x) => naga::Literal::AbstractFloat(*x),
            LiteralExpression::I32(x) => naga::Literal::I32(*x),
            LiteralExpression::U32(x) => naga::Literal::U32(*x),
            LiteralExpression::F32(x) => naga::Literal::F32(*x),
            LiteralExpression::F16(_) => return Err(E::F16NotSupported),
        })
    }
}

impl ToNaga<&UnaryExpression> {
    pub fn to_naga(&self, arena: &mut ExprArena) -> Result<naga::Expression, E> {
        let op = match self.operator {
            UnaryOperator::LogicalNegation => naga::UnaryOperator::LogicalNot,
            UnaryOperator::Negation => naga::UnaryOperator::Negate,
            UnaryOperator::BitwiseComplement => naga::UnaryOperator::BitwiseNot,
            UnaryOperator::AddressOf => todo!(),
            UnaryOperator::Indirection => todo!(),
        };

        let expr = ToNaga(&self.operand).to_naga(arena)?;
        let expr = arena.append(expr, ToNaga(self.operand.span()).to_naga());

        Ok(naga::Expression::Unary { op, expr })
    }
}

impl ToNaga<&ExpressionNode> {
    pub fn to_naga(&self, arena: &mut ExprArena) -> Result<naga::Expression, E> {
        Ok(match self.node() {
            Expression::Literal(expr) => naga::Expression::Literal(ToNaga(expr).to_naga()?),
            Expression::Parenthesized(expr) => todo!(),
            Expression::NamedComponent(expr) => todo!(),
            Expression::Indexing(expr) => todo!(),
            Expression::Unary(expr) => ToNaga(expr).to_naga(arena)?,
            Expression::Binary(expr) => todo!(),
            Expression::FunctionCall(expr) => todo!(),
            Expression::TypeOrIdentifier(expr) => todo!(),
        })
    }
}

impl ToNaga<&Span> {
    pub fn to_naga(&self) -> naga::Span {
        naga::Span::new(self.start as u32, self.end as u32)
    }
}

impl ToNaga<&AccessMode> {
    pub fn to_naga(&self) -> naga::StorageAccess {
        match self.deref() {
            AccessMode::Read => naga::StorageAccess::LOAD,
            AccessMode::Write => naga::StorageAccess::STORE,
            AccessMode::ReadWrite => naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
        }
    }
}

impl ToNaga<&AddressSpace> {
    pub fn to_naga(&self) -> naga::AddressSpace {
        match self.deref() {
            AddressSpace::Function => naga::AddressSpace::Function,
            AddressSpace::Private => naga::AddressSpace::Private,
            AddressSpace::Workgroup => naga::AddressSpace::WorkGroup,
            AddressSpace::Uniform => naga::AddressSpace::Uniform,
            AddressSpace::Storage(access_mode) => naga::AddressSpace::Storage {
                access: ToNaga(&access_mode.unwrap_or(AccessMode::Read)).to_naga(),
            },
            AddressSpace::Handle => naga::AddressSpace::Handle,
        }
    }
}

impl ToNaga<&Struct> {
    pub fn to_naga(&self, ctx: &mut Context, arena: &mut TypeArena) -> Result<naga::TypeInner, E> {
        let mut offset = 0;
        let members = self
            .members
            .iter()
            .map(|m| {
                let ty = self.ty.eval_ty(ctx)?;
                let size = self.attr_size(ctx)?.or_else(|| ty.size_of(ctx));
                let align = self.attr_align(ctx)?.or_else(|| ty.align_of(ctx));

                let name = Some(m.ident.to_string());
                let ty = arena.insert(ToNaga(&ty).to_naga(ctx, arena)?, naga::Span::UNDEFINED);
                let binding = None; // XXX: can someone explain to me why this is on struct??
                offset = round_up(align, offset);

                let mem = naga::StructMember {
                    name,
                    ty,
                    binding,
                    offset,
                };
                offset += size;
                Ok(mem)
            })
            .try_collect()?;

        Ok(naga::TypeInner::Struct {
            members,
            span: todo!(),
        })
    }
}

impl ToNaga<&TranslationUnit> {
    pub fn to_naga(&self) -> Result<naga::Module, E> {
        let mut types = UniqueArena::new();
        let mut constants = Arena::new();
        let mut overrides = Arena::new();
        let mut global_variables = Arena::new();
        let mut global_expressions = Arena::new();
        let mut functions = Arena::new();
        let mut entry_points = Vec::new();
        let mut diagnostic_filters = Arena::new();

        let mut ctx = Context::new(self);
        self.exec(&mut ctx)?; // add const-decls to the scope and eval const_asserts

        for decl in &self.global_declarations {
            match decl {
                GlobalDeclaration::Void => (),
                GlobalDeclaration::Declaration(decl) => {
                    let name = Some(decl.ident.to_string());
                    let ty = ToNaga(decl.ty.as_ref().unwrap()).to_naga(&mut ctx)?;
                    let ty = types.insert(ty, naga::Span::UNDEFINED);
                    let init = decl
                        .initializer
                        .as_ref()
                        .map(|init| {
                            let span = ToNaga(init.span()).to_naga();
                            let init = ToNaga(init).to_naga(&mut global_expressions)?;
                            let init = global_expressions.append(init, span);
                            Ok(init)
                        })
                        .transpose()?;
                    match decl.kind {
                        DeclarationKind::Const => {
                            let init = init.ok_or(E::UninitConst)?;
                            let constant = naga::Constant { name, ty, init };
                            constants.append(constant, naga::Span::UNDEFINED);
                        }
                        DeclarationKind::Override => {
                            let id = attr_id(&decl.attributes)?;
                            let override_ = naga::Override { name, id, ty, init };
                            overrides.append(override_, naga::Span::UNDEFINED);
                        }
                        DeclarationKind::Let => return Err(E::LetInMod),
                        DeclarationKind::Var(space) => {
                            let space = ToNaga(&space.unwrap_or(AddressSpace::Handle)).to_naga();
                            let binding = attr_group_binding(&decl.attributes)?
                                .map(|(group, binding)| naga::ResourceBinding { group, binding });

                            let constant = naga::GlobalVariable {
                                name,
                                space,
                                binding,
                                ty,
                                init,
                            };
                            global_variables.append(constant, naga::Span::UNDEFINED);
                        }
                    }
                }
                GlobalDeclaration::TypeAlias(_) => return Err(E::Unsupported("type aliases")),
                GlobalDeclaration::Struct(decl) => {}
                GlobalDeclaration::Function(decl) => todo!(),
                GlobalDeclaration::ConstAssert(decl) => todo!(),
            }
        }

        Ok(naga::Module {
            types,
            special_types: SpecialTypes::default(),
            constants,
            overrides,
            global_variables,
            global_expressions,
            functions,
            entry_points,
            diagnostic_filters,
            diagnostic_filter_leaf: None,
        })
    }
}
