//! Built-in type-generator and function templates.

use crate::{
    Error,
    inst::{Instance, LiteralInstance},
    syntax::{AccessMode, AddressSpace, Enumerant, SampledType, TexelFormat},
    ty::{TextureType, Ty, Type},
};

/// A single template parameter.
#[derive(Clone, Debug, PartialEq)]
pub enum TpltParam {
    Type(Type),
    Instance(Instance),
    Enumerant(Enumerant),
}

type E = Error;

// ------------------------
// TYPE-GENERATOR TEMPLATES
// ------------------------

pub struct ArrayTemplate {
    n: Option<usize>,
    ty: Type,
}

impl ArrayTemplate {
    pub fn new(ty: Type, n: Option<usize>) -> Self {
        Self { n, ty }
    }
    pub fn parse(tplt: &[TpltParam]) -> Result<ArrayTemplate, E> {
        let (ty, n) = match tplt {
            [TpltParam::Type(ty)] => Ok((ty.clone(), None)),
            [TpltParam::Type(ty), TpltParam::Instance(n)] => Ok((ty.clone(), Some(n.clone()))),
            _ => Err(E::TemplateArgs("array")),
        }?;
        if let Some(n) = n {
            let n = match n {
                Instance::Literal(LiteralInstance::AbstractInt(n)) => (n > 0).then_some(n as usize),
                Instance::Literal(LiteralInstance::I32(n)) => (n > 0).then_some(n as usize),
                Instance::Literal(LiteralInstance::U32(n)) => (n > 0).then_some(n as usize),
                #[cfg(feature = "naga-ext")]
                Instance::Literal(LiteralInstance::I64(n)) => (n > 0).then_some(n as usize),
                #[cfg(feature = "naga-ext")]
                Instance::Literal(LiteralInstance::U64(n)) => (n > 0).then_some(n as usize),
                _ => None,
            }
            .ok_or(E::Builtin(
                "the array element count must evaluate to a `u32` or a `i32` greater than `0`",
            ))?;
            Ok(ArrayTemplate { n: Some(n), ty })
        } else {
            Ok(ArrayTemplate { n: None, ty })
        }
    }
    pub fn ty(&self) -> Type {
        Type::Array(Box::new(self.ty.clone()), self.n)
    }
    pub fn inner_ty(&self) -> Type {
        self.ty.clone()
    }
    pub fn n(&self) -> Option<usize> {
        self.n
    }
}

#[cfg(feature = "naga-ext")]
pub struct BindingArrayTemplate {
    n: Option<usize>,
    ty: Type,
}

#[cfg(feature = "naga-ext")]
impl BindingArrayTemplate {
    pub fn parse(tplt: &[TpltParam]) -> Result<BindingArrayTemplate, E> {
        let (ty, n) = match tplt {
            [TpltParam::Type(ty)] => Ok((ty.clone(), None)),
            [TpltParam::Type(ty), TpltParam::Instance(n)] => Ok((ty.clone(), Some(n.clone()))),
            _ => Err(E::TemplateArgs("binding_array")),
        }?;
        if let Some(n) = n {
            let n = match n {
                Instance::Literal(LiteralInstance::AbstractInt(n)) => (n > 0).then_some(n as usize),
                Instance::Literal(LiteralInstance::I32(n)) => (n > 0).then_some(n as usize),
                Instance::Literal(LiteralInstance::U32(n)) => (n > 0).then_some(n as usize),
                Instance::Literal(LiteralInstance::I64(n)) => (n > 0).then_some(n as usize),
                Instance::Literal(LiteralInstance::U64(n)) => (n > 0).then_some(n as usize),
                _ => None,
            }
            .ok_or(E::Builtin(
                "the binding_array element count must evaluate to a `u32` or a `i32` greater than `0`",
            ))?;
            Ok(BindingArrayTemplate { n: Some(n), ty })
        } else {
            Ok(BindingArrayTemplate { n: None, ty })
        }
    }
    pub fn ty(&self) -> Type {
        Type::BindingArray(Box::new(self.ty.clone()), self.n)
    }
    pub fn inner_ty(&self) -> Type {
        self.ty.clone()
    }
    pub fn n(&self) -> Option<usize> {
        self.n
    }
}

pub struct VecTemplate {
    ty: Type,
}

impl VecTemplate {
    pub fn new_unchecked(ty: Type) -> Self {
        Self { ty }
    }
    pub fn parse(tplt: &[TpltParam]) -> Result<VecTemplate, E> {
        let ty = match tplt {
            [TpltParam::Type(ty)] => Ok(ty.clone()),
            _ => Err(E::TemplateArgs("vector")),
        }?;
        if ty.is_scalar() && ty.is_concrete() {
            Ok(VecTemplate { ty })
        } else {
            Err(Error::Builtin("vector template type must be a scalar"))
        }
    }
    pub fn ty(&self, n: u8) -> Type {
        Type::Vec(n, self.ty.clone().into())
    }
    pub fn inner_ty(&self) -> &Type {
        &self.ty
    }
}

pub struct MatTemplate {
    ty: Type,
}

impl MatTemplate {
    pub fn parse(tplt: &[TpltParam]) -> Result<MatTemplate, E> {
        let ty = match tplt {
            [TpltParam::Type(ty)] => Ok(ty.clone()),
            _ => Err(E::TemplateArgs("matrix")),
        }?;
        if ty.is_float() {
            Ok(MatTemplate { ty })
        } else {
            Err(Error::Builtin("matrix template type must be f32 or f16"))
        }
    }
    pub fn ty(&self, c: u8, r: u8) -> Type {
        Type::Mat(c, r, self.ty.clone().into())
    }

    pub fn inner_ty(&self) -> &Type {
        &self.ty
    }
}

pub struct PtrTemplate {
    pub space: AddressSpace,
    pub ty: Type,
    pub access: AccessMode,
}

impl PtrTemplate {
    pub fn parse(tplt: &[TpltParam]) -> Result<PtrTemplate, E> {
        let mut it = tplt.iter();
        match (
            it.next().cloned(),
            it.next().cloned(),
            it.next().cloned(),
            it.next(),
        ) {
            (
                Some(TpltParam::Enumerant(Enumerant::AddressSpace(space))),
                Some(TpltParam::Type(ty)),
                access,
                None,
            ) => {
                if !ty.is_storable() {
                    return Err(Error::Builtin("pointer type must be storable"));
                }
                let access = match access {
                    Some(TpltParam::Enumerant(Enumerant::AccessMode(access))) => Some(access),
                    _ => None,
                };
                // selecting the default access mode per address space.
                // reference: <https://www.w3.org/TR/WGSL/#address-space>
                let access = match (space, access) {
                    (AddressSpace::Function, Some(access))
                    | (AddressSpace::Private, Some(access))
                    | (AddressSpace::Workgroup, Some(access))
                    | (AddressSpace::Storage, Some(access)) => access,
                    (AddressSpace::Function, None)
                    | (AddressSpace::Private, None)
                    | (AddressSpace::Workgroup, None) => AccessMode::ReadWrite,
                    (AddressSpace::Uniform, Some(AccessMode::Read) | None) => AccessMode::Read,
                    (AddressSpace::Uniform, _) => {
                        return Err(Error::Builtin(
                            "pointer in uniform address space must have a `read` access mode",
                        ));
                    }
                    (AddressSpace::Storage, None) => AccessMode::Read,
                    (AddressSpace::Handle, _) => {
                        unreachable!("handle address space cannot be spelled")
                    }
                    #[cfg(feature = "naga-ext")]
                    (AddressSpace::Immediate, _) => {
                        todo!("immediate")
                    }
                };
                Ok(PtrTemplate { space, ty, access })
            }
            _ => Err(E::TemplateArgs("pointer")),
        }
    }

    pub fn ty(&self) -> Type {
        Type::Ptr(self.space, self.ty.clone().into(), self.access)
    }
}

pub struct AtomicTemplate {
    pub ty: Type,
}

impl AtomicTemplate {
    pub fn parse(tplt: &[TpltParam]) -> Result<AtomicTemplate, E> {
        let ty = match tplt {
            [TpltParam::Type(ty)] => Ok(ty.clone()),
            _ => Err(E::TemplateArgs("atomic")),
        }?;
        #[cfg(feature = "naga-ext")]
        if ty.is_f32() || ty.is_i64() || ty.is_u64() {
            return Ok(AtomicTemplate { ty });
        }
        if ty.is_i32() || ty.is_u32() {
            Ok(AtomicTemplate { ty })
        } else {
            Err(Error::Builtin("atomic template type must be an integer"))
        }
    }
    pub fn ty(&self) -> Type {
        Type::Atomic(self.ty.clone().into())
    }
    pub fn inner_ty(&self) -> Type {
        self.ty.clone()
    }
}

pub struct TextureTemplate {
    ty: TextureType,
}

impl TextureTemplate {
    pub fn parse(name: &str, tplt: &[TpltParam]) -> Result<TextureTemplate, E> {
        let ty = match name {
            "texture_1d" => TextureType::Sampled1D(Self::sampled_type(tplt)?),
            "texture_2d" => TextureType::Sampled2D(Self::sampled_type(tplt)?),
            "texture_2d_array" => TextureType::Sampled2DArray(Self::sampled_type(tplt)?),
            "texture_3d" => TextureType::Sampled3D(Self::sampled_type(tplt)?),
            "texture_cube" => TextureType::SampledCube(Self::sampled_type(tplt)?),
            "texture_cube_array" => TextureType::SampledCubeArray(Self::sampled_type(tplt)?),
            "texture_multisampled_2d" => TextureType::Multisampled2D(Self::sampled_type(tplt)?),
            "texture_storage_1d" => {
                let (tex, acc) = Self::texel_access(tplt)?;
                TextureType::Storage1D(tex, acc)
            }
            "texture_storage_2d" => {
                let (tex, acc) = Self::texel_access(tplt)?;
                TextureType::Storage2D(tex, acc)
            }
            "texture_storage_2d_array" => {
                let (tex, acc) = Self::texel_access(tplt)?;
                TextureType::Storage2DArray(tex, acc)
            }
            "texture_storage_3d" => {
                let (tex, acc) = Self::texel_access(tplt)?;
                TextureType::Storage3D(tex, acc)
            }
            #[cfg(feature = "naga-ext")]
            "texture_1d_array" => TextureType::Sampled1DArray(Self::sampled_type(tplt)?),
            #[cfg(feature = "naga-ext")]
            "texture_storage_1d_array" => {
                let (tex, acc) = Self::texel_access(tplt)?;
                TextureType::Storage1DArray(tex, acc)
            }
            #[cfg(feature = "naga-ext")]
            "texture_multisampled_2d_array" => {
                TextureType::Multisampled2DArray(Self::sampled_type(tplt)?)
            }
            _ => return Err(E::Builtin("not a templated texture type")),
        };
        Ok(Self { ty })
    }
    fn sampled_type(tplt: &[TpltParam]) -> Result<SampledType, E> {
        match tplt {
            [TpltParam::Type(ty)] => ty.try_into(),
            [_] => Err(Error::Builtin(
                "texture sampled type must be `i32`, `u32` or `f32`",
            )),
            _ => Err(Error::Builtin(
                "sampled texture types take a single template parameter",
            )),
        }
    }
    fn texel_access(tplt: &[TpltParam]) -> Result<(TexelFormat, AccessMode), E> {
        match tplt {
            [
                TpltParam::Enumerant(Enumerant::TexelFormat(texel)),
                TpltParam::Enumerant(Enumerant::AccessMode(access)),
            ] => Ok((*texel, *access)),
            _ => Err(Error::Builtin(
                "storage texture types take two template parameters",
            )),
        }
    }
    pub fn ty(&self) -> TextureType {
        self.ty.clone()
    }
}

pub struct BitcastTemplate {
    ty: Type,
}

impl BitcastTemplate {
    pub fn parse(tplt: &[TpltParam]) -> Result<BitcastTemplate, E> {
        let ty = match tplt {
            [TpltParam::Type(ty)] => Ok(ty.clone()),
            _ => Err(E::TemplateArgs("bitcast")),
        }?;
        if ty.is_numeric() || ty.is_vec() && ty.inner_ty().is_numeric() {
            Ok(BitcastTemplate { ty })
        } else {
            Err(Error::Builtin(
                "bitcast template type must be a numeric scalar or numeric vector",
            ))
        }
    }
    pub fn ty(&self) -> &Type {
        &self.ty
    }
}
