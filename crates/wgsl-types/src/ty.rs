//! WGSL [`Type`]s.

use std::str::FromStr;

use crate::{Error, Instance, inst::*, syntax::*};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructMemberType {
    pub name: String,
    pub ty: Type,
    pub size: Option<u32>,
    pub align: Option<u32>,
}

impl StructMemberType {
    pub fn new(name: String, ty: Type) -> Self {
        Self {
            name,
            ty,
            size: None,
            align: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructType {
    pub name: String,
    pub members: Vec<StructMemberType>,
}

impl From<StructType> for Type {
    fn from(value: StructType) -> Self {
        Self::Struct(Box::new(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureType {
    // sampled
    Sampled1D(SampledType),
    Sampled2D(SampledType),
    Sampled2DArray(SampledType),
    Sampled3D(SampledType),
    SampledCube(SampledType),
    SampledCubeArray(SampledType),
    // multisampled
    Multisampled2D(SampledType),
    DepthMultisampled2D,
    // external
    External,
    // storage
    Storage1D(TexelFormat, AccessMode),
    Storage2D(TexelFormat, AccessMode),
    Storage2DArray(TexelFormat, AccessMode),
    Storage3D(TexelFormat, AccessMode),
    // depth
    Depth2D,
    Depth2DArray,
    DepthCube,
    DepthCubeArray,
    #[cfg(feature = "naga-ext")]
    Sampled1DArray(SampledType),
    #[cfg(feature = "naga-ext")]
    Storage1DArray(TexelFormat, AccessMode),
    #[cfg(feature = "naga-ext")]
    Multisampled2DArray(SampledType),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextureDimensions {
    D1,
    D2,
    D3,
}

impl TextureType {
    pub fn dimensions(&self) -> TextureDimensions {
        match self {
            Self::Sampled1D(_) | Self::Storage1D(_, _) => TextureDimensions::D1,
            Self::Sampled2D(_)
            | Self::Sampled2DArray(_)
            | Self::SampledCube(_)
            | Self::SampledCubeArray(_)
            | Self::Multisampled2D(_)
            | Self::Depth2D
            | Self::Depth2DArray
            | Self::DepthCube
            | Self::DepthCubeArray
            | Self::DepthMultisampled2D
            | Self::Storage2D(_, _)
            | Self::Storage2DArray(_, _)
            | Self::External => TextureDimensions::D2,
            Self::Sampled3D(_) | Self::Storage3D(_, _) => TextureDimensions::D3,
            #[cfg(feature = "naga-ext")]
            Self::Sampled1DArray(_) | Self::Storage1DArray(_, _) => TextureDimensions::D1,
            #[cfg(feature = "naga-ext")]
            Self::Multisampled2DArray(_) => TextureDimensions::D2,
        }
    }
    pub fn sampled_type(&self) -> Option<SampledType> {
        match self {
            TextureType::Sampled1D(st) => Some(*st),
            TextureType::Sampled2D(st) => Some(*st),
            TextureType::Sampled2DArray(st) => Some(*st),
            TextureType::Sampled3D(st) => Some(*st),
            TextureType::SampledCube(st) => Some(*st),
            TextureType::SampledCubeArray(st) => Some(*st),
            TextureType::Multisampled2D(st) => Some(*st),
            TextureType::DepthMultisampled2D => None,
            TextureType::External => None,
            TextureType::Storage1D(_, _) => None,
            TextureType::Storage2D(_, _) => None,
            TextureType::Storage2DArray(_, _) => None,
            TextureType::Storage3D(_, _) => None,
            TextureType::Depth2D => None,
            TextureType::Depth2DArray => None,
            TextureType::DepthCube => None,
            TextureType::DepthCubeArray => None,
            #[cfg(feature = "naga-ext")]
            TextureType::Sampled1DArray(st) => Some(*st),
            #[cfg(feature = "naga-ext")]
            TextureType::Storage1DArray(_, _) => None,
            #[cfg(feature = "naga-ext")]
            TextureType::Multisampled2DArray(st) => Some(*st),
        }
    }
    pub fn channel_type(&self) -> SampledType {
        match self {
            TextureType::Sampled1D(st) => *st,
            TextureType::Sampled2D(st) => *st,
            TextureType::Sampled2DArray(st) => *st,
            TextureType::Sampled3D(st) => *st,
            TextureType::SampledCube(st) => *st,
            TextureType::SampledCubeArray(st) => *st,
            TextureType::Multisampled2D(st) => *st,
            TextureType::DepthMultisampled2D => SampledType::F32,
            TextureType::External => SampledType::F32,
            TextureType::Storage1D(f, _) => f.channel_type(),
            TextureType::Storage2D(f, _) => f.channel_type(),
            TextureType::Storage2DArray(f, _) => f.channel_type(),
            TextureType::Storage3D(f, _) => f.channel_type(),
            TextureType::Depth2D => SampledType::F32,
            TextureType::Depth2DArray => SampledType::F32,
            TextureType::DepthCube => SampledType::F32,
            TextureType::DepthCubeArray => SampledType::F32,
            #[cfg(feature = "naga-ext")]
            TextureType::Sampled1DArray(st) => *st,
            #[cfg(feature = "naga-ext")]
            TextureType::Storage1DArray(f, _) => f.channel_type(),
            #[cfg(feature = "naga-ext")]
            TextureType::Multisampled2DArray(st) => *st,
        }
    }
    /// NOTE: a `texture_depth_multisampled_2d` is *not* considered a depth texture.
    pub fn is_depth(&self) -> bool {
        matches!(
            self,
            TextureType::Depth2D
                | TextureType::Depth2DArray
                | TextureType::DepthCube
                | TextureType::DepthCubeArray
        )
    }
    pub fn is_storage(&self) -> bool {
        match self {
            TextureType::Storage1D(_, _)
            | TextureType::Storage2D(_, _)
            | TextureType::Storage2DArray(_, _)
            | TextureType::Storage3D(_, _) => true,
            #[cfg(feature = "naga-ext")]
            TextureType::Storage1DArray(_, _) => true,
            _ => false,
        }
    }
    pub fn is_sampled(&self) -> bool {
        match self {
            TextureType::Sampled1D(_)
            | TextureType::Sampled2D(_)
            | TextureType::Sampled2DArray(_)
            | TextureType::Sampled3D(_)
            | TextureType::SampledCube(_)
            | TextureType::SampledCubeArray(_) => true,
            #[cfg(feature = "naga-ext")]
            TextureType::Sampled1DArray(_) => true,
            _ => false,
        }
    }
    pub fn is_arrayed(&self) -> bool {
        match self {
            TextureType::Sampled2DArray(_)
            | TextureType::SampledCubeArray(_)
            | TextureType::Storage2DArray(_, _)
            | TextureType::Depth2DArray
            | TextureType::DepthCubeArray => true,
            #[cfg(feature = "naga-ext")]
            TextureType::Sampled1DArray(_)
            | TextureType::Storage1DArray(_, _)
            | TextureType::Multisampled2DArray(_) => true,
            _ => false,
        }
    }
    pub fn is_multisampled(&self) -> bool {
        match self {
            TextureType::Multisampled2D(_) | TextureType::DepthMultisampled2D => true,
            #[cfg(feature = "naga-ext")]
            TextureType::Multisampled2DArray(_) => true,
            _ => false,
        }
    }
    pub fn is_cube(&self) -> bool {
        matches!(
            self,
            TextureType::SampledCube(_)
                | TextureType::SampledCubeArray(_)
                | TextureType::DepthCube
                | TextureType::DepthCubeArray
        )
    }
}

impl TryFrom<&Type> for SampledType {
    type Error = Error;

    fn try_from(value: &Type) -> Result<Self, Self::Error> {
        match value {
            Type::I32 => Ok(SampledType::I32),
            Type::U32 => Ok(SampledType::U32),
            Type::F32 => Ok(SampledType::F32),
            _ => Err(Error::SampledType(value.clone())),
        }
    }
}

impl From<SampledType> for Type {
    fn from(value: SampledType) -> Self {
        match value {
            SampledType::I32 => Type::I32,
            SampledType::U32 => Type::U32,
            SampledType::F32 => Type::F32,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SamplerType {
    Sampler,
    SamplerComparison,
}

impl FromStr for SamplerType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sampler" => Ok(Self::Sampler),
            "sampler_comparison" => Ok(Self::SamplerComparison),
            _ => Err(()),
        }
    }
}

/// WGSL type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Bool,
    AbstractInt,
    AbstractFloat,
    I32,
    U32,
    F32,
    F16,
    Struct(Box<StructType>),
    Array(Box<Type>, Option<usize>),
    Vec(u8, Box<Type>),
    Mat(u8, u8, Box<Type>),
    Atomic(Box<Type>),
    Ptr(AddressSpace, Box<Type>, AccessMode),
    Ref(AddressSpace, Box<Type>, AccessMode),
    Texture(TextureType),
    Sampler(SamplerType),
    /// This variant is used by wgsl-analyzer and other type-checking tools when
    /// a type is unknown. It is not an regular WGSL type.
    Unknown,
    #[cfg(feature = "naga-ext")]
    I64,
    #[cfg(feature = "naga-ext")]
    U64,
    #[cfg(feature = "naga-ext")]
    F64,
    #[cfg(feature = "naga-ext")]
    BindingArray(Box<Type>, Option<usize>),
    #[cfg(feature = "naga-ext")]
    RayQuery(Option<AccelerationStructureFlags>),
    #[cfg(feature = "naga-ext")]
    AccelerationStructure(Option<AccelerationStructureFlags>),
}

impl Type {
    /// Reference: <https://www.w3.org/TR/WGSL/#scalar>
    pub fn is_scalar(&self) -> bool {
        match self {
            Type::Bool
            | Type::AbstractInt
            | Type::AbstractFloat
            | Type::I32
            | Type::U32
            | Type::F32
            | Type::F16 => true,
            #[cfg(feature = "naga-ext")]
            Type::I64 | Type::U64 | Type::F64 => true,
            _ => false,
        }
    }

    /// Reference: <https://www.w3.org/TR/WGSL/#numeric-scalar>
    pub fn is_numeric(&self) -> bool {
        match self {
            Type::AbstractInt
            | Type::AbstractFloat
            | Type::I32
            | Type::U32
            | Type::F32
            | Type::F16 => true,
            #[cfg(feature = "naga-ext")]
            Type::I64 | Type::U64 | Type::F64 => true,
            _ => false,
        }
    }

    /// Reference: <https://www.w3.org/TR/WGSL/#integer-scalar>
    pub fn is_integer(&self) -> bool {
        match self {
            Type::AbstractInt | Type::I32 | Type::U32 => true,
            #[cfg(feature = "naga-ext")]
            Type::I64 | Type::U64 => true,
            _ => false,
        }
    }

    /// Is a signed numeric type.
    pub fn is_signed(&self) -> bool {
        match self {
            Type::AbstractInt | Type::AbstractFloat | Type::I32 | Type::F32 | Type::F16 => true,
            #[cfg(feature = "naga-ext")]
            Type::I64 | Type::F64 => true,
            _ => false,
        }
    }

    /// Reference: <https://www.w3.org/TR/WGSL/#floating-point-types>
    pub fn is_float(&self) -> bool {
        match self {
            Type::AbstractFloat | Type::F32 | Type::F16 => true,
            #[cfg(feature = "naga-ext")]
            Type::F64 => true,
            _ => false,
        }
    }

    /// Reference: <https://www.w3.org/TR/WGSL/#abstract-types>
    pub fn is_abstract(&self) -> bool {
        match self {
            Type::AbstractInt => true,
            Type::AbstractFloat => true,
            Type::Array(ty, _) | Type::Vec(_, ty) | Type::Mat(_, _, ty) => ty.is_abstract(),
            _ => false,
        }
    }

    pub fn is_concrete(&self) -> bool {
        match self {
            Type::Unknown => false,
            _ => !self.is_abstract(),
        }
    }

    /// Reference: <https://www.w3.org/TR/WGSL/#storable-types>
    pub fn is_storable(&self) -> bool {
        self.is_concrete()
            && match self {
                Type::Bool
                | Type::I32
                | Type::U32
                | Type::F32
                | Type::F16
                | Type::Struct(_)
                | Type::Array(_, _)
                | Type::Vec(_, _)
                | Type::Mat(_, _, _)
                | Type::Atomic(_) => true,
                #[cfg(feature = "naga-ext")]
                Type::I64 | Type::U64 | Type::F64 => true,
                _ => false,
            }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Type::Array(_, _))
    }
    pub fn is_vec(&self) -> bool {
        matches!(self, Type::Vec(_, _))
    }
    pub fn is_i32(&self) -> bool {
        matches!(self, Type::I32)
    }
    pub fn is_u32(&self) -> bool {
        matches!(self, Type::U32)
    }
    pub fn is_f32(&self) -> bool {
        matches!(self, Type::F32)
    }
    #[cfg(feature = "naga-ext")]
    pub fn is_i64(&self) -> bool {
        matches!(self, Type::I64)
    }
    #[cfg(feature = "naga-ext")]
    pub fn is_u64(&self) -> bool {
        matches!(self, Type::U64)
    }
    #[cfg(feature = "naga-ext")]
    pub fn is_f64(&self) -> bool {
        matches!(self, Type::F64)
    }
    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }
    pub fn is_mat(&self) -> bool {
        matches!(self, Type::Mat(_, _, _))
    }
    pub fn is_abstract_int(&self) -> bool {
        matches!(self, Type::AbstractInt)
    }

    pub fn unwrap_atomic(self) -> Box<Type> {
        match self {
            Type::Atomic(ty) => ty,
            val => panic!("called `Type::unwrap_atomic()` on a `{val}` value"),
        }
    }

    pub fn unwrap_struct(self) -> Box<StructType> {
        match self {
            Type::Struct(ty) => ty,
            val => panic!("called `Type::unwrap_struct()` on a `{val}` value"),
        }
    }

    pub fn unwrap_vec(self) -> (u8, Box<Type>) {
        match self {
            Type::Vec(size, ty) => (size, ty),
            val => panic!("called `Type::unwrap_vec()` on a `{val}` value"),
        }
    }
}

pub trait Ty {
    /// get the type of an instance.
    fn ty(&self) -> Type;

    /// get the inner type of an instance (not recursive).
    ///
    /// e.g. the inner type of `array<vec3<u32>>` is `vec3<u32>`.
    fn inner_ty(&self) -> Type {
        self.ty()
    }
}

impl Ty for Type {
    fn ty(&self) -> Type {
        self.clone()
    }

    fn inner_ty(&self) -> Type {
        match self {
            Type::Bool => self.clone(),
            Type::AbstractInt => self.clone(),
            Type::AbstractFloat => self.clone(),
            Type::I32 => self.clone(),
            Type::U32 => self.clone(),
            Type::F32 => self.clone(),
            Type::F16 => self.clone(),
            Type::Struct(_) => self.clone(),
            Type::Array(ty, _) => ty.ty(),
            Type::Vec(_, ty) => ty.ty(),
            Type::Mat(_, _, ty) => ty.ty(),
            Type::Atomic(ty) => ty.ty(),
            Type::Ptr(_, ty, _) => ty.ty(),
            Type::Ref(_, ty, _) => ty.ty(),
            Type::Texture(_) => self.clone(),
            Type::Sampler(_) => self.clone(),
            Type::Unknown => self.clone(),
            #[cfg(feature = "naga-ext")]
            Type::I64 => self.clone(),
            #[cfg(feature = "naga-ext")]
            Type::U64 => self.clone(),
            #[cfg(feature = "naga-ext")]
            Type::F64 => self.clone(),
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(ty, _) => ty.ty(),
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(_) => self.clone(),
            #[cfg(feature = "naga-ext")]
            Type::AccelerationStructure(_) => self.clone(),
        }
    }
}

impl Ty for Instance {
    fn ty(&self) -> Type {
        match self {
            Instance::Literal(l) => l.ty(),
            Instance::Struct(s) => s.ty(),
            Instance::Array(a) => a.ty(),
            Instance::Vec(v) => v.ty(),
            Instance::Mat(m) => m.ty(),
            Instance::Ptr(p) => p.ty(),
            Instance::Ref(r) => r.ty(),
            Instance::Atomic(a) => a.ty(),
            Instance::Deferred(t) => t.ty(),
        }
    }
    fn inner_ty(&self) -> Type {
        match self {
            Instance::Literal(l) => l.inner_ty(),
            Instance::Struct(s) => s.inner_ty(),
            Instance::Array(a) => a.inner_ty(),
            Instance::Vec(v) => v.inner_ty(),
            Instance::Mat(m) => m.inner_ty(),
            Instance::Ptr(p) => p.inner_ty(),
            Instance::Ref(r) => r.inner_ty(),
            Instance::Atomic(a) => a.inner_ty(),
            Instance::Deferred(t) => t.inner_ty(),
        }
    }
}

impl Ty for LiteralInstance {
    fn ty(&self) -> Type {
        match self {
            LiteralInstance::Bool(_) => Type::Bool,
            LiteralInstance::AbstractInt(_) => Type::AbstractInt,
            LiteralInstance::AbstractFloat(_) => Type::AbstractFloat,
            LiteralInstance::I32(_) => Type::I32,
            LiteralInstance::U32(_) => Type::U32,
            LiteralInstance::F32(_) => Type::F32,
            LiteralInstance::F16(_) => Type::F16,
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => Type::I64,
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Type::U64,
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(_) => Type::F64,
        }
    }
}

impl Ty for StructInstance {
    fn ty(&self) -> Type {
        self.ty.clone().into()
    }
}

impl Ty for ArrayInstance {
    fn ty(&self) -> Type {
        Type::Array(
            Box::new(self.inner_ty().clone()),
            (!self.runtime_sized).then_some(self.n()),
        )
    }
    fn inner_ty(&self) -> Type {
        self.get(0).unwrap().ty()
    }
}

impl Ty for VecInstance {
    fn ty(&self) -> Type {
        Type::Vec(self.n() as u8, Box::new(self.inner_ty()))
    }
    fn inner_ty(&self) -> Type {
        self.get(0).unwrap().ty()
    }
}

impl Ty for MatInstance {
    fn ty(&self) -> Type {
        Type::Mat(self.c() as u8, self.r() as u8, Box::new(self.inner_ty()))
    }
    fn inner_ty(&self) -> Type {
        self.get(0, 0).unwrap().ty()
    }
}

impl Ty for PtrInstance {
    fn ty(&self) -> Type {
        Type::Ptr(
            self.ptr.space,
            Box::new(self.ptr.ty.clone()),
            self.ptr.access,
        )
    }
}

impl Ty for RefInstance {
    fn ty(&self) -> Type {
        Type::Ref(self.space, Box::new(self.ty.clone()), self.access)
    }
}

impl Ty for AtomicInstance {
    fn ty(&self) -> Type {
        Type::Atomic(self.inner_ty().into())
    }
    fn inner_ty(&self) -> Type {
        self.inner().ty()
    }
}
