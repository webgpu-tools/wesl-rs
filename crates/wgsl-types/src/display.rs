use std::fmt::Display;

use itertools::Itertools;

use crate::{
    CallSignature,
    inst::{
        ArrayInstance, AtomicInstance, Instance, LiteralInstance, MatInstance, MemView,
        PtrInstance, RefInstance, StructInstance, VecInstance,
    },
    syntax::Enumerant,
    tplt::TpltParam,
    ty::{SamplerType, TextureType, Ty, Type},
};

impl Display for TpltParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TpltParam::Type(ty) => write!(f, "{ty}"),
            TpltParam::Instance(inst) => write!(f, "{inst}"),
            TpltParam::Enumerant(name) => write!(f, "{name}"),
        }
    }
}

impl Display for CallSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = &self.name;
        let tplt = self.tplt.as_ref().map(|tplt| tplt.iter().format(", "));
        let args = self.args.iter().format(", ");

        if let Some(tplt) = tplt {
            write!(f, "{name}<{tplt}>({args})")
        } else {
            write!(f, "{name}({args})")
        }
    }
}

impl Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Instance::Literal(inst) => write!(f, "{inst}"),
            Instance::Struct(inst) => write!(f, "{inst}"),
            Instance::Array(inst) => write!(f, "{inst}"),
            Instance::Vec(inst) => write!(f, "{inst}"),
            Instance::Mat(inst) => write!(f, "{inst}"),
            Instance::Ptr(inst) => write!(f, "{inst}"),
            Instance::Ref(inst) => write!(f, "{inst}"),
            Instance::Atomic(inst) => write!(f, "{inst}"),
            Instance::Opaque(ty) => write!(f, "__opaque<{ty}>"),
        }
    }
}

impl Display for LiteralInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LiteralInstance::Bool(lit) => write!(f, "{lit}"),
            LiteralInstance::AbstractInt(lit) => write!(f, "{lit}"),
            LiteralInstance::AbstractFloat(lit) => write!(f, "{lit:?}"), // using the Debug formatter to print the trailing .0 in floats representing integers. because format!("{}", 3.0f32) == "3"
            LiteralInstance::I32(lit) => write!(f, "{lit}i"),
            LiteralInstance::U32(lit) => write!(f, "{lit}u"),
            LiteralInstance::F32(lit) => write!(f, "{lit}f"),
            LiteralInstance::F16(lit) => write!(f, "{lit}h"),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(lit) => write!(f, "{lit}li"),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(lit) => write!(f, "{lit}lu"),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(lit) => write!(f, "{lit}lf"),
        }
    }
}

impl Display for StructInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = &self.ty.name;
        let comps = self
            .members
            .iter()
            .map(|inst| format!("{inst}"))
            .format(", ");
        write!(f, "{name}({comps})")
    }
}

impl Display for ArrayInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let comps = self.iter().format(", ");
        write!(f, "array({comps})")
    }
}

impl Display for VecInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.n();
        let comps = self.iter().format(", ");
        write!(f, "vec{n}({comps})")
    }
}

impl Display for MatInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = self.c();
        let r = self.r();
        let comps = (0..c).map(|i| self.col(i).unwrap()).format(", ");
        write!(f, "mat{c}x{r}({comps})")
    }
}

impl Display for Enumerant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Enumerant::AccessMode(access_mode) => write!(f, "{access_mode}"),
            Enumerant::AddressSpace(address_space) => write!(f, "{address_space}"),
            Enumerant::TexelFormat(texel_format) => write!(f, "{texel_format}"),
            #[cfg(feature = "naga-ext")]
            Enumerant::AccelerationStructureFlags(flags) => write!(f, "{flags}"),
        }
    }
}

impl Display for PtrInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let space = &self.ptr.space;
        let ty = &self.ptr.ty;
        let access = &self.ptr.access;
        let val = self.ptr.read().expect("invalid reference");
        write!(f, "ptr<{space}, {ty}, {access}>({val})")
    }
}

impl Display for RefInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let space = &self.space;
        let ty = &self.ty;
        let access = &self.access;
        let val = self.read().expect("invalid reference");
        write!(f, "ref<{space}, {ty}, {access}>({val})")
    }
}

impl Display for MemView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn fmt_view(f: &mut std::fmt::Formatter<'_>, view: &MemView) -> std::fmt::Result {
            match view {
                MemView::Whole => Ok(()),
                MemView::Member(m, v) => {
                    write!(f, ":{m}")?;
                    fmt_view(f, v)
                }
                MemView::Index(i, v) => {
                    write!(f, ":{i}")?;
                    fmt_view(f, v)
                }
            }
        }

        fmt_view(f, self)
    }
}

impl Display for AtomicInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ty = &self.inner_ty();
        let val = self.inner();
        write!(f, "atomic<{ty}>({val})")
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Bool => write!(f, "bool"),
            Type::AbstractInt => write!(f, "AbstractInt"),
            Type::AbstractFloat => write!(f, "AbstractFloat"),
            Type::I32 => write!(f, "i32"),
            Type::U32 => write!(f, "u32"),
            Type::F32 => write!(f, "f32"),
            Type::F16 => write!(f, "f16"),
            Type::Struct(s) => write!(f, "{}", s.name),
            Type::Array(ty, Some(n)) => write!(f, "array<{ty}, {n}>"),
            Type::Array(ty, None) => write!(f, "array<{ty}>"),
            Type::Vec(n, ty) => write!(f, "vec{n}<{ty}>"),
            Type::Mat(m, n, ty) => write!(f, "mat{m}x{n}<{ty}>"),
            Type::Atomic(ty) => write!(f, "atomic<{ty}>"),
            Type::Ptr(a_s, ty, a_m) => write!(f, "ptr<{a_s}, {ty}, {a_m}>"),
            Type::Ref(a_s, ty, a_m) => write!(f, "ref<{a_s}, {ty}, {a_m}>"),
            Type::Texture(texture_type) => texture_type.fmt(f),
            Type::Sampler(sampler_type) => sampler_type.fmt(f),
            #[cfg(feature = "naga-ext")]
            Type::I64 => write!(f, "i64"),
            #[cfg(feature = "naga-ext")]
            Type::U64 => write!(f, "u64"),
            #[cfg(feature = "naga-ext")]
            Type::F64 => write!(f, "f64"),
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(ty, Some(n)) => write!(f, "binding_array<{ty}, {n}>"),
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(ty, None) => write!(f, "binding_array<{ty}>"),
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(None) => write!(f, "ray_query"),
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(Some(_)) => write!(f, "ray_query<vertex_return>"),
            #[cfg(feature = "naga-ext")]
            Type::AccelerationStructure(None) => write!(f, "acceleration_structure"),
            #[cfg(feature = "naga-ext")]
            Type::AccelerationStructure(Some(_)) => {
                write!(f, "acceleration_structure<vertex_return>")
            }
        }
    }
}

impl Display for TextureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureType::Sampled1D(sampled) => write!(f, "texture_1d<{sampled}>"),
            TextureType::Sampled2D(sampled) => write!(f, "texture_2d<{sampled}>"),
            TextureType::Sampled2DArray(sampled) => write!(f, "texture_2d_array<{sampled}>"),
            TextureType::Sampled3D(sampled) => write!(f, "texture_3d<{sampled}>"),
            TextureType::SampledCube(sampled) => write!(f, "texture_cube<{sampled}>"),
            TextureType::SampledCubeArray(sampled) => write!(f, "texture_cube_array<{sampled}>"),
            TextureType::Multisampled2D(sampled) => write!(f, "texture_multisampled_2d<{sampled}>"),
            TextureType::DepthMultisampled2D => write!(f, "texture_depth_multisampled_2d"),
            TextureType::External => write!(f, "texture_external<>"),
            TextureType::Storage1D(texel, access) => {
                write!(f, "texture_storage_1d<{texel}, {access}>")
            }
            TextureType::Storage2D(texel, access) => {
                write!(f, "texture_storage_2d<{texel}, {access}>")
            }
            TextureType::Storage2DArray(texel, access) => {
                write!(f, "texture_storage_2d_array<{texel}, {access}>")
            }
            TextureType::Storage3D(texel, access) => {
                write!(f, "texture_storage_3d<{texel}, {access}>")
            }
            TextureType::Depth2D => write!(f, "texture_depth_2d"),
            TextureType::Depth2DArray => write!(f, "texture_depth_2d_array"),
            TextureType::DepthCube => write!(f, "texture_depth_cube"),
            TextureType::DepthCubeArray => write!(f, "texture_depth_cube_array"),
            #[cfg(feature = "naga-ext")]
            TextureType::Sampled1DArray(sampled) => write!(f, "texture_1d_array<{sampled}>"),
            #[cfg(feature = "naga-ext")]
            TextureType::Storage1DArray(texel, access) => {
                write!(f, "texture_storage_1d_array<{texel}, {access}>")
            }
            #[cfg(feature = "naga-ext")]
            TextureType::Multisampled2DArray(sampled) => {
                write!(f, "texture_multisampled_2d_array<{sampled}>")
            }
        }
    }
}

impl Display for SamplerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SamplerType::Sampler => write!(f, "sampler"),
            SamplerType::SamplerComparison => write!(f, "sampler_comparison"),
        }
    }
}
