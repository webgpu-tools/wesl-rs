use std::{collections::HashMap, sync::LazyLock};

use wgsl_parse::syntax::*;
use wgsl_types::idents::iter_builtin_idents;
#[cfg(feature = "eval")]
use wgsl_types::{
    syntax::SampledType,
    ty::{SamplerType, TextureType, Type},
};

/// All built-in names as [`Ident`]s.
///
/// Using these idents allow better use-count tracking and referencing.
pub static BUILTIN_IDENTS: LazyLock<HashMap<&str, Ident>> = LazyLock::new(|| {
    HashMap::from_iter(iter_builtin_idents().map(|id| (id, Ident::new(id.to_string()))))
});

/// Get a built-in WGSL name as [`Ident`].
///
/// Using these idents allow better use-count tracking and referencing.
pub fn builtin_ident(name: &str) -> Option<&'static Ident> {
    BUILTIN_IDENTS.get(name)
}

#[cfg(feature = "eval")]
pub trait BuiltinIdent {
    fn builtin_ident(&self) -> Option<&'static Ident>;
}

#[cfg(feature = "eval")]
impl BuiltinIdent for Type {
    fn builtin_ident(&self) -> Option<&'static Ident> {
        match self {
            Type::Bool => builtin_ident("bool"),
            Type::AbstractInt => builtin_ident("__AbstractInt"),
            Type::AbstractFloat => builtin_ident("__AbstractFloat"),
            Type::I32 => builtin_ident("i32"),
            Type::U32 => builtin_ident("u32"),
            Type::F32 => builtin_ident("f32"),
            Type::F16 => builtin_ident("f16"),
            Type::Struct(_) => None,
            Type::Array(_, _) => builtin_ident("array"),
            Type::Vec(n, _) => match n {
                2 => builtin_ident("vec2"),
                3 => builtin_ident("vec3"),
                4 => builtin_ident("vec4"),
                _ => unreachable!("vec must be 2 3 or 4 components"),
            },
            Type::Mat(c, r, _) => match (c, r) {
                (2, 2) => builtin_ident("mat2x2"),
                (2, 3) => builtin_ident("mat2x3"),
                (2, 4) => builtin_ident("mat2x4"),
                (3, 2) => builtin_ident("mat3x2"),
                (3, 3) => builtin_ident("mat3x3"),
                (3, 4) => builtin_ident("mat3x4"),
                (4, 2) => builtin_ident("mat4x2"),
                (4, 3) => builtin_ident("mat4x3"),
                (4, 4) => builtin_ident("mat4x4"),
                _ => unreachable!("mat must be 2 3 or 4 components"),
            },
            Type::Atomic(_) => builtin_ident("atomic"),
            Type::Ptr(_, _, _) => builtin_ident("ptr"),
            Type::Ref(_, _, _) => builtin_ident("__ref"),
            Type::Texture(texture_type) => texture_type.builtin_ident(),
            Type::Sampler(sampler_type) => sampler_type.builtin_ident(),
            Type::Unknown => builtin_ident("__unknown"),
            #[cfg(feature = "naga-ext")]
            Type::I64 => builtin_ident("i64"),
            #[cfg(feature = "naga-ext")]
            Type::U64 => builtin_ident("u64"),
            #[cfg(feature = "naga-ext")]
            Type::F64 => builtin_ident("f64"),
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(_, _) => builtin_ident("binding_array"),
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(_) => builtin_ident("ray_query"),
            #[cfg(feature = "naga-ext")]
            Type::AccelerationStructure(_) => builtin_ident("acceleration_structure"),
        }
    }
}

#[cfg(feature = "eval")]
impl BuiltinIdent for TextureType {
    fn builtin_ident(&self) -> Option<&'static Ident> {
        builtin_ident(match self {
            Self::Sampled1D(_) => "texture_1d",
            Self::Sampled2D(_) => "texture_2d",
            Self::Sampled2DArray(_) => "texture_2d_array",
            Self::Sampled3D(_) => "texture_3d",
            Self::SampledCube(_) => "texture_cube",
            Self::SampledCubeArray(_) => "texture_cube_array",
            Self::Multisampled2D(_) => "texture_multisampled_2d",
            Self::DepthMultisampled2D => "texture_depth_multisampled_2d",
            Self::External => "texture_external",
            Self::Storage1D(_, _) => "texture_storage_1d",
            Self::Storage2D(_, _) => "texture_storage_2d",
            Self::Storage2DArray(_, _) => "texture_storage_2d_array",
            Self::Storage3D(_, _) => "texture_storage_3d",
            Self::Depth2D => "texture_depth_2d",
            Self::Depth2DArray => "texture_depth_2d_array",
            Self::DepthCube => "texture_depth_cube",
            Self::DepthCubeArray => "texture_depth_cube_array",
            #[cfg(feature = "naga-ext")]
            Self::Sampled1DArray(_) => "texture_1d_array",
            #[cfg(feature = "naga-ext")]
            Self::Storage1DArray(_, _) => "texture_storage_1d_array",
            #[cfg(feature = "naga-ext")]
            Self::Multisampled2DArray(_) => "texture_multisampled_2d_array",
        })
    }
}

#[cfg(feature = "eval")]
impl BuiltinIdent for SamplerType {
    fn builtin_ident(&self) -> Option<&'static Ident> {
        match self {
            SamplerType::Sampler => builtin_ident("sampler"),
            SamplerType::SamplerComparison => builtin_ident("sampler_comparison"),
        }
    }
}

#[cfg(feature = "eval")]
impl BuiltinIdent for SampledType {
    fn builtin_ident(&self) -> Option<&'static Ident> {
        match self {
            Self::I32 => builtin_ident("i32"),
            Self::U32 => builtin_ident("u32"),
            Self::F32 => builtin_ident("f32"),
        }
    }
}

#[cfg(feature = "eval")]
impl BuiltinIdent for AddressSpace {
    fn builtin_ident(&self) -> Option<&'static Ident> {
        match self {
            Self::Function => builtin_ident("function"),
            Self::Private => builtin_ident("private"),
            Self::Workgroup => builtin_ident("workgroup"),
            Self::Uniform => builtin_ident("uniform"),
            Self::Storage => builtin_ident("storage"),
            Self::Handle => None,
            #[cfg(feature = "naga-ext")]
            Self::Immediate => builtin_ident("immediate"),
            #[cfg(feature = "naga-ext")]
            Self::TaskPayload => builtin_ident("task_payload"),
        }
    }
}

#[cfg(feature = "eval")]
impl BuiltinIdent for AccessMode {
    fn builtin_ident(&self) -> Option<&'static Ident> {
        match self {
            Self::Read => builtin_ident("read"),
            Self::Write => builtin_ident("write"),
            Self::ReadWrite => builtin_ident("read_write"),
            #[cfg(feature = "naga-ext")]
            Self::Atomic => builtin_ident("atomic"),
        }
    }
}
