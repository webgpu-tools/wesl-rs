//! Type-checking and computation of the return type of built-in functions.
//!
//! Functions bear the same name as the WGSL counterpart.
//! Functions that take template parameters are suffixed with `_t` and take `tplt_*` arguments.
//!
//! ### Warning
//!
//! Type-checking of some functions is incomplete.

#![allow(non_snake_case)]

use crate::{
    CallSignature, Error,
    conv::{Convert, convert_all_ty, convert_ty},
    syntax::*,
    tplt::{BitcastTemplate, TpltParam},
    ty::{StructMemberType, StructType, TextureDimensions, TextureType, Ty, Type},
};

type E = Error;

/// Compute the return type of calling a built-in function.
///
/// The arguments must be [loaded][Type::loaded].
///
/// Does not include constructor built-ins, see [`type_ctor`][super::type_ctor].
///
/// Some functions are still TODO, see [`call`][super::call] for the list of functions and statuses.
pub fn type_builtin_fn(
    name: &str,
    tplt: Option<&[TpltParam]>,
    args: &[Type],
) -> Result<Option<Type>, E> {
    let err = || {
        E::Signature(CallSignature {
            name: name.to_string(),
            tplt: tplt.map(|t| t.to_vec()),
            args: args.to_vec(),
        })
    };

    match (name, args) {
        // bitcast
        ("bitcast", [a]) if let Some(tplt) = tplt => {
            let tplt = BitcastTemplate::parse(tplt)?;
            bitcast_t(tplt.ty(), a).map(Some)
        }
        // logical
        ("all", [a]) => all(a).map(Some),
        ("any", [a]) => any(a).map(Some),
        ("select", [a1, a2, a3]) => select(a1, a2, a3).map(Some),
        // array
        ("arrayLength", [a]) => arrayLength(a).map(Some),
        // numeric
        ("abs", [a]) => abs(a).map(Some),
        ("acos", [a]) => acos(a).map(Some),
        ("acosh", [a]) => acosh(a).map(Some),
        ("asin", [a]) => asin(a).map(Some),
        ("asinh", [a]) => asinh(a).map(Some),
        ("atan", [a]) => atan(a).map(Some),
        ("atanh", [a]) => atanh(a).map(Some),
        ("atan2", [a1, a2]) => atan2(a1, a2).map(Some),
        ("ceil", [a]) => ceil(a).map(Some),
        ("clamp", [a1, a2, a3]) => clamp(a1, a2, a3).map(Some),
        ("cos", [a]) => cos(a).map(Some),
        ("cosh", [a]) => cosh(a).map(Some),
        ("countLeadingZeros", [a]) => countLeadingZeros(a).map(Some),
        ("countOneBits", [a]) => countOneBits(a).map(Some),
        ("countTrailingZeros", [a]) => countTrailingZeros(a).map(Some),
        ("cross", [a1, a2]) => cross(a1, a2).map(Some),
        ("degrees", [a]) => degrees(a).map(Some),
        ("determinant", [a]) => determinant(a).map(Some),
        ("distance", [a1, a2]) => distance(a1, a2).map(Some),
        ("dot", [a1, a2]) => dot(a1, a2).map(Some),
        ("dot4U8Packed", [a1, a2]) => dot4U8Packed(a1, a2).map(Some),
        ("dot4I8Packed", [a1, a2]) => dot4I8Packed(a1, a2).map(Some),
        ("exp", [a]) => exp(a).map(Some),
        ("exp2", [a]) => exp2(a).map(Some),
        ("extractBits", [a1, a2, a3]) => extractBits(a1, a2, a3).map(Some),
        ("faceForward", [a1, a2, a3]) => faceForward(a1, a2, a3).map(Some),
        ("firstLeadingBit", [a]) => firstLeadingBit(a).map(Some),
        ("firstTrailingBit", [a]) => firstTrailingBit(a).map(Some),
        ("floor", [a]) => floor(a).map(Some),
        ("fma", [a1, a2, a3]) => fma(a1, a2, a3).map(Some),
        ("fract", [a]) => fract(a).map(Some),
        ("frexp", [a]) => frexp(a).map(Some),
        ("insertBits", [a1, a2, a3, a4]) => insertBits(a1, a2, a3, a4).map(Some),
        ("inverseSqrt", [a]) => inverseSqrt(a).map(Some),
        ("ldexp", [a1, a2]) => ldexp(a1, a2).map(Some),
        ("length", [a]) => length(a).map(Some),
        ("log", [a]) => log(a).map(Some),
        ("log2", [a]) => log2(a).map(Some),
        ("max", [a1, a2]) => max(a1, a2).map(Some),
        ("min", [a1, a2]) => min(a1, a2).map(Some),
        ("mix", [a1, a2, a3]) => mix(a1, a2, a3).map(Some),
        ("modf", [a]) => modf(a).map(Some),
        ("normalize", [a]) => normalize(a).map(Some),
        ("pow", [a1, a2]) => pow(a1, a2).map(Some),
        ("quantizeToF16", [a]) => quantizeToF16(a).map(Some),
        ("radians", [a]) => radians(a).map(Some),
        ("reflect", [a1, a2]) => reflect(a1, a2).map(Some),
        ("refract", [a1, a2, a3]) => refract(a1, a2, a3).map(Some),
        ("reverseBits", [a]) => reverseBits(a).map(Some),
        ("round", [a]) => round(a).map(Some),
        ("saturate", [a]) => saturate(a).map(Some),
        ("sign", [a]) => sign(a).map(Some),
        ("sin", [a]) => sin(a).map(Some),
        ("sinh", [a]) => sinh(a).map(Some),
        ("smoothstep", [a1, a2, a3]) => smoothstep(a1, a2, a3).map(Some),
        ("sqrt", [a]) => sqrt(a).map(Some),
        ("step", [a1, a2]) => step(a1, a2).map(Some),
        ("tan", [a]) => tan(a).map(Some),
        ("tanh", [a]) => tanh(a).map(Some),
        ("transpose", [a]) => transpose(a).map(Some),
        ("trunc", [a]) => trunc(a).map(Some),
        // derivative
        ("dpdx", [a]) => dpdx(a).map(Some),
        ("dpdxCoarse", [a]) => dpdxCoarse(a).map(Some),
        ("dpdxFine", [a]) => dpdxFine(a).map(Some),
        ("dpdy", [a]) => dpdy(a).map(Some),
        ("dpdyCoarse", [a]) => dpdyCoarse(a).map(Some),
        ("dpdyFine", [a]) => dpdyFine(a).map(Some),
        ("fwidth", [a]) => fwidth(a).map(Some),
        ("fwidthCoarse", [a]) => fwidthCoarse(a).map(Some),
        ("fwidthFine", [a]) => fwidthFine(a).map(Some),
        // texture
        // TODO: check arguments for texture functions
        ("textureDimensions", [a1]) => textureDimensions(a1, None).map(Some),
        ("textureGather", [a1, a2, a3]) => textureGather(a1, a2, a3, None, None, None).map(Some),
        ("textureGather", [a1, a2, a3, a4]) => {
            textureGather(a1, a2, a3, Some(a4), None, None).map(Some)
        }
        ("textureGather", [a1, a2, a3, a4, a5]) => {
            textureGather(a1, a2, a3, Some(a4), Some(a5), None).map(Some)
        }
        ("textureGather", [a1, a2, a3, a4, a5, a6]) => {
            textureGather(a1, a2, a3, Some(a4), Some(a5), Some(a6)).map(Some)
        }
        ("textureGatherCompare", [a1, a2, a3, a4]) => {
            textureGatherCompare(a1, a2, a3, a4, None, None).map(Some)
        }
        ("textureGatherCompare", [a1, a2, a3, a4, a5]) => {
            textureGatherCompare(a1, a2, a3, a4, Some(a5), None).map(Some)
        }
        ("textureGatherCompare", [a1, a2, a3, a4, a5, a6]) => {
            textureGatherCompare(a1, a2, a3, a4, Some(a5), Some(a6)).map(Some)
        }
        ("textureLoad", [a1, a2]) => textureLoad(a1, a2, None, None).map(Some),
        ("textureLoad", [a1, a2, a3]) => textureLoad(a1, a2, Some(a3), None).map(Some),
        ("textureLoad", [a1, a2, a3, a4]) => textureLoad(a1, a2, Some(a3), Some(a4)).map(Some),
        ("textureNumLayers", [a]) => textureNumLayers(a).map(Some),
        ("textureNumLevels", [a]) => textureNumLevels(a).map(Some),
        ("textureNumSamples", [a]) => textureNumSamples(a).map(Some),
        ("textureSample", [a1, a2, a3]) => textureSample(a1, a2, a3, None, None).map(Some),
        ("textureSample", [a1, a2, a3, a4]) => textureSample(a1, a2, a3, Some(a4), None).map(Some),
        ("textureSample", [a1, a2, a3, a4, a5]) => {
            textureSample(a1, a2, a3, Some(a4), Some(a5)).map(Some)
        }
        ("textureSampleBias", [a1, a2, a3, a4]) => {
            textureSampleBias(a1, a2, a3, a4, None, None).map(Some)
        }
        ("textureSampleBias", [a1, a2, a3, a4, a5]) => {
            textureSampleBias(a1, a2, a3, a4, Some(a5), None).map(Some)
        }
        ("textureSampleBias", [a1, a2, a3, a4, a5, a6]) => {
            textureSampleBias(a1, a2, a3, a4, Some(a5), Some(a6)).map(Some)
        }
        ("textureSampleCompare", [a1, a2, a3, a4]) => {
            textureSampleCompare(a1, a2, a3, a4, None, None).map(Some)
        }
        ("textureSampleCompare", [a1, a2, a3, a4, a5]) => {
            textureSampleCompare(a1, a2, a3, a4, Some(a5), None).map(Some)
        }
        ("textureSampleCompare", [a1, a2, a3, a4, a5, a6]) => {
            textureSampleCompare(a1, a2, a3, a4, Some(a5), Some(a6)).map(Some)
        }
        ("textureSampleCompareLevel", [a1, a2, a3, a4]) => {
            textureSampleCompareLevel(a1, a2, a3, a4, None, None).map(Some)
        }
        ("textureSampleCompareLevel", [a1, a2, a3, a4, a5]) => {
            textureSampleCompareLevel(a1, a2, a3, a4, Some(a5), None).map(Some)
        }
        ("textureSampleCompareLevel", [a1, a2, a3, a4, a5, a6]) => {
            textureSampleCompareLevel(a1, a2, a3, a4, Some(a5), Some(a6)).map(Some)
        }
        ("textureSampleGrad", [a1, a2, a3, a4, a5]) => {
            textureSampleGrad(a1, a2, a3, a4, a5, None, None).map(Some)
        }
        ("textureSampleGrad", [a1, a2, a3, a4, a5, a6]) => {
            textureSampleGrad(a1, a2, a3, a4, a5, Some(a6), None).map(Some)
        }
        ("textureSampleGrad", [a1, a2, a3, a4, a5, a6, a7]) => {
            textureSampleGrad(a1, a2, a3, a4, a5, Some(a6), Some(a7)).map(Some)
        }
        ("textureSampleLevel", [a1, a2, a3, a4]) => {
            textureSampleLevel(a1, a2, a3, a4, None, None).map(Some)
        }
        ("textureSampleLevel", [a1, a2, a3, a4, a5]) => {
            textureSampleLevel(a1, a2, a3, a4, Some(a5), None).map(Some)
        }
        ("textureSampleLevel", [a1, a2, a3, a4, a5, a6]) => {
            textureSampleLevel(a1, a2, a3, a4, Some(a5), Some(a6)).map(Some)
        }
        ("textureSampleBaseClampToEdge", [a1, a2, a3]) => {
            textureSampleBaseClampToEdge(a1, a2, a3).map(Some)
        }
        ("textureStore", [a1, a2, a3]) => textureStore(a1, a2, a3, None).map(|()| None),
        ("textureStore", [a1, a2, a3, a4]) => textureStore(a1, a2, a3, Some(a4)).map(|()| None),
        // atomic
        ("atomicLoad", [a]) => atomicLoad(a).map(Some),
        ("atomicStore", [a1, a2]) => atomicStore(a1, a2).map(|()| None),
        ("atomicAdd", [a1, a2]) => atomicAdd(a1, a2).map(Some),
        ("atomicSub", [a1, a2]) => atomicSub(a1, a2).map(Some),
        ("atomicMax", [a1, a2]) => atomicMax(a1, a2).map(Some),
        ("atomicMin", [a1, a2]) => atomicMin(a1, a2).map(Some),
        ("atomicAnd", [a1, a2]) => atomicAnd(a1, a2).map(Some),
        ("atomicOr", [a1, a2]) => atomicOr(a1, a2).map(Some),
        ("atomicXor", [a1, a2]) => atomicXor(a1, a2).map(Some),
        ("atomicExchange", [a1, a2]) => atomicExchange(a1, a2).map(Some),
        ("atomicCompareExchangeWeak", [a1, a2, a3]) => {
            atomicCompareExchangeWeak(a1, a2, a3).map(Some)
        }
        // packing
        ("pack4x8snorm", [a]) => pack4x8snorm(a).map(Some),
        ("pack4x8unorm", [a]) => pack4x8unorm(a).map(Some),
        ("pack4xI8", [a]) => pack4xI8(a).map(Some),
        ("pack4xU8", [a]) => pack4xU8(a).map(Some),
        ("pack4xI8Clamp", [a]) => pack4xI8Clamp(a).map(Some),
        ("pack4xU8Clamp", [a]) => pack4xU8Clamp(a).map(Some),
        ("pack2x16snorm", [a]) => pack2x16snorm(a).map(Some),
        ("pack2x16unorm", [a]) => pack2x16unorm(a).map(Some),
        ("pack2x16float", [a]) => pack2x16float(a).map(Some),
        ("unpack4x8snorm", [a]) => unpack4x8snorm(a).map(Some),
        ("unpack4x8unorm", [a]) => unpack4x8unorm(a).map(Some),
        ("unpack4xI8", [a]) => unpack4xI8(a).map(Some),
        ("unpack4xU8", [a]) => unpack4xU8(a).map(Some),
        ("unpack2x16snorm", [a]) => unpack2x16snorm(a).map(Some),
        ("unpack2x16unorm", [a]) => unpack2x16unorm(a).map(Some),
        ("unpack2x16float", [a]) => unpack2x16float(a).map(Some),
        // synchronization
        ("storageBarrier", []) => Ok(None),
        ("textureBarrier", []) => Ok(None),
        ("workgroupBarrier", []) => Ok(None),
        ("workgroupUniformLoad", [Type::Ptr(AddressSpace::Workgroup, t, _)]) => {
            Ok(Some(*t.clone()))
        }
        // subgroup
        ("subgroupAdd", [a]) => subgroupAdd(a).map(Some),
        ("subgroupExclusiveAdd", [a]) => subgroupExclusiveAdd(a).map(Some),
        ("subgroupInclusiveAdd", [a]) => subgroupInclusiveAdd(a).map(Some),
        ("subgroupAll", [a]) => subgroupAll(a).map(Some),
        ("subgroupAnd", [a]) => subgroupAnd(a).map(Some),
        ("subgroupAny", [a]) => subgroupAny(a).map(Some),
        ("subgroupBallot", [a]) => subgroupBallot(Some(a)).map(Some),
        #[cfg(feature = "naga-ext")]
        ("subgroupBallot", []) => subgroupBallot(None).map(Some),
        ("subgroupBroadcast", [a1, a2]) => subgroupBroadcast(a1, a2).map(Some),
        ("subgroupBroadcastFirst", [a]) => subgroupBroadcastFirst(a).map(Some),
        ("subgroupElect", []) => subgroupElect().map(Some),
        ("subgroupMax", [a]) => subgroupMax(a).map(Some),
        ("subgroupMin", [a]) => subgroupMin(a).map(Some),
        ("subgroupMul", [a]) => subgroupMul(a).map(Some),
        ("subgroupExclusiveMul", [a]) => subgroupExclusiveMul(a).map(Some),
        ("subgroupInclusiveMul", [a]) => subgroupInclusiveMul(a).map(Some),
        ("subgroupOr", [a]) => subgroupOr(a).map(Some),
        ("subgroupShuffle", [a1, a2]) => subgroupShuffle(a1, a2).map(Some),
        ("subgroupShuffleDown", [a1, a2]) => subgroupShuffleDown(a1, a2).map(Some),
        ("subgroupShuffleUp", [a1, a2]) => subgroupShuffleUp(a1, a2).map(Some),
        ("subgroupShuffleXor", [a1, a2]) => subgroupShuffleXor(a1, a2).map(Some),
        ("subgroupXor", [a]) => subgroupXor(a).map(Some),
        // quad
        ("quadBroadcast", [a1, a2]) => quadBroadcast(a1, a2).map(Some),
        ("quadSwapDiagonal", [a]) => quadSwapDiagonal(a).map(Some),
        ("quadSwapX", [a]) => quadSwapX(a).map(Some),
        ("quadSwapY", [a]) => quadSwapY(a).map(Some),
        // naga ray queries extension
        #[cfg(feature = "naga-ext")]
        ("rayQueryInitialize", [a1, a2, a3]) => rayQueryInitialize(a1, a2, a3).map(|()| None),
        #[cfg(feature = "naga-ext")]
        ("rayQueryProceed", [a]) => rayQueryProceed(a).map(Some),
        #[cfg(feature = "naga-ext")]
        ("rayQueryGenerateIntersection", [a1, a2]) => {
            rayQueryGenerateIntersection(a1, a2).map(|()| None)
        }
        #[cfg(feature = "naga-ext")]
        ("rayQueryConfirmIntersection", [a]) => rayQueryConfirmIntersection(a).map(|()| None),
        #[cfg(feature = "naga-ext")]
        ("rayQueryTerminate", [a]) => rayQueryTerminate(a).map(|()| None),
        #[cfg(feature = "naga-ext")]
        ("rayQueryGetCommittedIntersection", [a]) => rayQueryGetCommittedIntersection(a).map(Some),
        #[cfg(feature = "naga-ext")]
        ("rayQueryGetCandidateIntersection", [a]) => rayQueryGetCandidateIntersection(a).map(Some),
        #[cfg(feature = "naga-ext")]
        ("getCommittedHitVertexPositions", [a]) => getCommittedHitVertexPositions(a).map(Some),
        #[cfg(feature = "naga-ext")]
        ("getCandidateHitVertexPositions", [a]) => getCandidateHitVertexPositions(a).map(Some),
        _ => Err(err()),
    }
}

// ---------------------
// BUILT-IN RETURN TYPES
// ---------------------

pub(crate) fn frexp_struct_name(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::AbstractFloat => Some("__frexp_result_abstract"),
        Type::F32 => Some("__frexp_result_f32"),
        Type::F16 => Some("__frexp_result_f16"),
        #[cfg(feature = "naga-ext")]
        Type::F64 => Some("__frexp_result_f64"),
        Type::Vec(n, ty) => match (n, &**ty) {
            (2, Type::AbstractFloat) => Some("__frexp_result_vec2_abstract"),
            (2, Type::F32) => Some("__frexp_result_vec2_f32"),
            (2, Type::F16) => Some("__frexp_result_vec2_f16"),
            (3, Type::AbstractFloat) => Some("__frexp_result_vec3_abstract"),
            (3, Type::F32) => Some("__frexp_result_vec3_f32"),
            (3, Type::F16) => Some("__frexp_result_vec3_f16"),
            (4, Type::AbstractFloat) => Some("__frexp_result_vec4_abstract"),
            (4, Type::F32) => Some("__frexp_result_vec4_f32"),
            (4, Type::F16) => Some("__frexp_result_vec4_f16"),
            #[cfg(feature = "naga-ext")]
            (2, Type::F64) => Some("__frexp_result_vec2_f64"),
            #[cfg(feature = "naga-ext")]
            (3, Type::F64) => Some("__frexp_result_vec3_f64"),
            #[cfg(feature = "naga-ext")]
            (4, Type::F64) => Some("__frexp_result_vec4_f64"),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn frexp_struct_type(ty: &Type) -> Option<StructType> {
    frexp_struct_name(ty).map(|name| {
        let exp_inner_ty = if ty.is_abstract() {
            Type::AbstractInt
        } else {
            Type::I32
        };
        let exp_ty = match ty {
            Type::Vec(n, _) => Type::Vec(*n, Box::new(exp_inner_ty)),
            _ => exp_inner_ty,
        };
        StructType {
            name: name.to_string(),
            members: vec![
                StructMemberType::new("fract".to_string(), ty.clone()),
                StructMemberType::new("exp".to_string(), exp_ty),
            ],
        }
    })
}

pub(crate) fn modf_struct_name(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::AbstractFloat => Some("__modf_result_abstract"),
        Type::F32 => Some("__modf_result_f32"),
        Type::F16 => Some("__modf_result_f16"),
        #[cfg(feature = "naga-ext")]
        Type::F64 => Some("__modf_result_f64"),
        Type::Vec(n, ty) => match (n, &**ty) {
            (2, Type::AbstractFloat) => Some("__modf_result_vec2_abstract"),
            (2, Type::F32) => Some("__modf_result_vec2_f32"),
            (2, Type::F16) => Some("__modf_result_vec2_f16"),
            (3, Type::AbstractFloat) => Some("__modf_result_vec3_abstract"),
            (3, Type::F32) => Some("__modf_result_vec3_f32"),
            (3, Type::F16) => Some("__modf_result_vec3_f16"),
            (4, Type::AbstractFloat) => Some("__modf_result_vec4_abstract"),
            (4, Type::F32) => Some("__modf_result_vec4_f32"),
            (4, Type::F16) => Some("__modf_result_vec4_f16"),
            #[cfg(feature = "naga-ext")]
            (2, Type::F64) => Some("__modf_result_vec2_f64"),
            #[cfg(feature = "naga-ext")]
            (3, Type::F64) => Some("__modf_result_vec3_f64"),
            #[cfg(feature = "naga-ext")]
            (4, Type::F64) => Some("__modf_result_vec4_f64"),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn atomic_compare_exchange_struct_type(ty: &Type) -> StructType {
    StructType {
        name: "__atomic_compare_exchange_result".to_string(),
        members: vec![
            StructMemberType::new("old_value".to_string(), ty.clone()),
            StructMemberType::new("exchanged".to_string(), Type::Bool),
        ],
    }
}

pub(crate) fn modf_struct_type(ty: &Type) -> Option<StructType> {
    modf_struct_name(ty).map(|name| StructType {
        name: name.to_string(),
        members: vec![
            StructMemberType::new("fract".to_string(), ty.clone()),
            StructMemberType::new("whole".to_string(), ty.clone()),
        ],
    })
}

#[cfg(feature = "naga-ext")]
#[allow(unused)]
pub(crate) fn ray_desc_struct_type() -> StructType {
    StructType {
        name: "RayDesc".to_string(),
        members: vec![
            StructMemberType::new("flags".to_string(), Type::U32),
            StructMemberType::new("cull_mask".to_string(), Type::U32),
            StructMemberType::new("t_min".to_string(), Type::F32),
            StructMemberType::new("t_max".to_string(), Type::F32),
            StructMemberType::new("origin".to_string(), Type::Vec(3, Box::new(Type::F32))),
            StructMemberType::new("dir".to_string(), Type::Vec(3, Box::new(Type::F32))),
        ],
    }
}

#[cfg(feature = "naga-ext")]
pub(crate) fn ray_intersection_struct_type() -> StructType {
    StructType {
        name: "RayIntersection".to_string(),
        members: vec![
            StructMemberType::new("kind".to_string(), Type::U32),
            StructMemberType::new("t".to_string(), Type::F32),
            StructMemberType::new("instance_custom_data".to_string(), Type::U32),
            StructMemberType::new("instance_index".to_string(), Type::U32),
            StructMemberType::new("sbt_record_offset".to_string(), Type::U32),
            StructMemberType::new("geometry_index".to_string(), Type::U32),
            StructMemberType::new("primitive_index".to_string(), Type::U32),
            StructMemberType::new(
                "barycentrics".to_string(),
                Type::Vec(2, Box::new(Type::F32)),
            ),
            StructMemberType::new("front_face".to_string(), Type::Bool),
            StructMemberType::new(
                "object_to_world".to_string(),
                Type::Mat(4, 3, Box::new(Type::F32)),
            ),
            StructMemberType::new(
                "world_to_object".to_string(),
                Type::Mat(4, 3, Box::new(Type::F32)),
            ),
        ],
    }
}

// utility predicates for `T or vecN<T>` constraints.
fn inner_is_float(ty: &Type) -> bool {
    ty.is_float() || matches!(ty, Type::Vec(_, t) if t.is_float())
}
fn inner_is_numeric(ty: &Type) -> bool {
    ty.is_numeric() || matches!(ty, Type::Vec(_, t) if t.is_numeric())
}
fn inner_is_integer(ty: &Type) -> bool {
    ty.is_integer() || matches!(ty, Type::Vec(_, t) if t.is_integer())
}
fn inner_is_bool(ty: &Type) -> bool {
    ty.is_bool() || matches!(ty, Type::Vec(_, t) if t.is_bool())
}

// -------
// BITCAST
// -------
// reference: <https://www.w3.org/TR/WGSL/#bit-reinterp-builtin-functions>

/// `bitcast<T>()` builtin function.
///
/// we assume `tplt_ty` is a concrete numeric scalar or concrete numeric vector.
///
/// XXX: the spec explicitly provides an overload `bitcast<u32>(AbstractInt)`,
/// but not with `i32`. In principle, automatic conversion can take care of that.
/// So why is there an explicit overload?
///
/// Reference: <https://www.w3.org/TR/WGSL/#bitcast-builtin>
pub fn bitcast_t(tplt_ty: &Type, e: &Type) -> Result<Type, E> {
    if tplt_ty.size_of() != e.concretize().size_of() {
        Err(E::Builtin(
            "`bitcast` argument must have the same byte length as the template type",
        ))
    } else if inner_is_numeric(e) {
        Ok(tplt_ty.clone())
    } else {
        Err(E::Builtin(
            "`bitcast` expects a numeric scalar or numeric vector argument",
        ))
    }
}

// -------
// LOGICAL
// -------
// reference: <https://www.w3.org/TR/WGSL/#logical-builtin-functions>

/// `all()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#all-builtin>
pub fn all(e: &Type) -> Result<Type, E> {
    if inner_is_bool(e) {
        Ok(Type::Bool)
    } else {
        Err(E::Builtin(
            "`all` expects a boolean or vector of boolean argument",
        ))
    }
}

/// `any()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#any-builtin>
pub fn any(e: &Type) -> Result<Type, E> {
    if inner_is_bool(e) {
        Ok(Type::Bool)
    } else {
        Err(E::Builtin(
            "`any` expects a boolean or vector of boolean argument",
        ))
    }
}

/// `select()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#select-builtin>
pub fn select(f: &Type, t: &Type, cond: &Type) -> Result<Type, E> {
    let ty = convert_ty(f, t).ok_or(E::Builtin(
        "`select` 1st and 2nd arguments are incompatible",
    ))?;

    match (ty, cond) {
        (Type::Vec(n_ty, _), Type::Vec(n_cond, t)) => {
            if !t.is_bool() {
                Err(E::Builtin(
                    "`select` 3rd argument must be a boolean or vector of boolean",
                ))
            } else if n_ty != n_cond {
                Err(E::Builtin(
                    "`select` 3rd vector argument has incorrect dimensions",
                ))
            } else {
                Ok(ty.clone())
            }
        }
        (ty, Type::Bool) if ty.is_scalar() || ty.is_vec() => Ok(ty.clone()),
        (_, Type::Bool) => Err(E::Builtin(
            "`select` 1st and 2nd arguments must be a scalar or vector",
        )),
        (_, _) => Err(E::Builtin(
            "`select` 3rd argument must be a boolean or vector of boolean",
        )),
    }
}

// -----
// ARRAY
// -----
// reference: <https://www.w3.org/TR/WGSL/#array-builtin-functions>

/// `arrayLength()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#arrayLength-builtin>
pub fn arrayLength(p: &Type) -> Result<Type, E> {
    match p {
        Type::Ptr(AddressSpace::Storage, t, a_m)
            if a_m.is_read() && matches!(**t, Type::Array(_, None)) =>
        {
            Ok(Type::U32)
        }
        _ => Err(E::Builtin(
            "`arrayLength` argument must be a pointer to runtime-sized array, with `storage` address space and `read` or `read_write` access mode",
        )),
    }
}

// -------
// NUMERIC
// -------
// reference: <https://www.w3.org/TR/WGSL/#numeric-builtin-function>

/// `abs()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#abs-float-builtin>
pub fn abs(e: &Type) -> Result<Type, E> {
    inner_is_numeric(e).then_some(e.clone()).ok_or(E::Builtin(
        "`abs` argument must be a numeric scalar or vector",
    ))
}

/// `acos()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#acos-builtin>
pub fn acos(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`acos` argument must be a float scalar or vector",
    ))
}

/// `acosh()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#acosh-builtin>
pub fn acosh(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`acosh` argument must be a float scalar or vector",
    ))
}

/// `asin()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#asin-builtin>
pub fn asin(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`asin` argument must be a float scalar or vector",
    ))
}

/// `asinh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#asinh-builtin>
pub fn asinh(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`asinh` argument must be a float scalar or vector",
    ))
}

/// `atan()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atan-builtin>
pub fn atan(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`atan` argument must be a float scalar or vector",
    ))
}

/// `atanh()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#atanh-builtin>
pub fn atanh(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`atanh` argument must be a float scalar or vector",
    ))
}

/// `atan2()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atan2-builtin>
pub fn atan2(y: &Type, x: &Type) -> Result<Type, E> {
    let ty = convert_ty(y, x).ok_or(E::Builtin("`atan2 arguments are incompatible`"))?;
    inner_is_float(ty).then_some(ty.clone()).ok_or(E::Builtin(
        "`atan2` expects float scalar or vector arguments",
    ))
}

/// `ceil()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#ceil-builtin>
pub fn ceil(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`ceil` argument must be a float scalar or vector",
    ))
}

/// `clamp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#clamp>
pub fn clamp(e: &Type, low: &Type, high: &Type) -> Result<Type, E> {
    let ty =
        convert_all_ty([e, low, high]).ok_or(E::Builtin("`clamp` arguments are incompatible"))?;
    if inner_is_numeric(ty) {
        Ok(ty.clone())
    } else {
        Err(E::Builtin(
            "`clamp` expects three numeric scalar or vector arguments",
        ))
    }
}

/// `cos()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#cos-builtin>
pub fn cos(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`cos` argument must be a float scalar or vector",
    ))
}

/// `cosh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#cosh-builtin>
pub fn cosh(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`cosh` argument must be a float scalar or vector",
    ))
}

/// `countLeadingZeros()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#countLeadingZeros-builtin>
pub fn countLeadingZeros(e: &Type) -> Result<Type, E> {
    inner_is_integer(e)
        .then_some(e.concretize())
        .ok_or(E::Builtin(
            "`countLeadingZeros` argument must be a integer scalar or vector",
        ))
}

/// `countOneBits()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#countOneBits-builtin>
pub fn countOneBits(e: &Type) -> Result<Type, E> {
    inner_is_integer(e)
        .then_some(e.concretize())
        .ok_or(E::Builtin(
            "`countOneBits` argument must be a integer scalar or vector",
        ))
}

/// `countTrailingZeros()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#countTrailingZeros-builtin>
pub fn countTrailingZeros(e: &Type) -> Result<Type, E> {
    inner_is_integer(e)
        .then_some(e.concretize())
        .ok_or(E::Builtin(
            "`countTrailingZeros` argument must be a integer scalar or vector",
        ))
}

/// `cross()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#cross-builtin>
pub fn cross(a: &Type, b: &Type) -> Result<Type, E> {
    let ty = convert_ty(a, b).ok_or(E::Builtin("`cross` arguments are incompatible"))?;
    match ty {
        Type::Vec(3, t) if t.is_float() => Ok(ty.clone()),
        _ => Err(E::Builtin(
            "`cross` expects two 3-component float vector arguments",
        )),
    }
}

/// `degrees()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#degrees-builtin>
pub fn degrees(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`degrees` expects a float scalar or vector argument",
    ))
}

/// `determinant()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#determinant-builtin>
pub fn determinant(e: &Type) -> Result<Type, E> {
    match e {
        Type::Mat(c, r, t) if c == r => Ok(*t.clone()),
        _ => Err(E::Builtin("`determinant` expects a square matrix argument")),
    }
}

/// `distance()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#distance-builtin>
pub fn distance(e1: &Type, e2: &Type) -> Result<Type, E> {
    let ty = convert_ty(e1, e2).ok_or(E::Builtin("`distance` arguments are incompatible"))?;
    inner_is_float(ty)
        .then_some(ty.inner_ty())
        .ok_or(E::Builtin(
            "`distance` expects two float scalar or vector arguments",
        ))
}

/// `dot()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dot-builtin>
pub fn dot(e1: &Type, e2: &Type) -> Result<Type, E> {
    let ty = convert_ty(e1, e2).ok_or(E::Builtin("`dot` arguments are incompatible"))?;
    match ty {
        Type::Vec(_, t) if t.is_numeric() => Ok(*t.clone()),
        _ => Err(E::Builtin(
            "`dot` expects two numeric scalar or vector arguments",
        )),
    }
}

/// `dot4U8Packed()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dot4U8Packed-builtin>
pub fn dot4U8Packed(e1: &Type, e2: &Type) -> Result<Type, E> {
    if e1.is_convertible_to(&Type::U32) && e2.is_convertible_to(&Type::U32) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`dot4U8Packed` expects two u32 arguments"))
    }
}

/// `dot4I8Packed()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dot4I8Packed-builtin>
pub fn dot4I8Packed(e1: &Type, e2: &Type) -> Result<Type, E> {
    if e1.is_convertible_to(&Type::U32) && e2.is_convertible_to(&Type::U32) {
        Ok(Type::I32)
    } else {
        Err(E::Builtin("`dot4I8Packed` expects two u32 arguments"))
    }
}

/// `exp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#exp-builtin>
pub fn exp(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`exp` argument must be a float scalar or vector",
    ))
}

/// `exp2()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#exp2-builtin>
pub fn exp2(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`exp2` argument must be a float scalar or vector",
    ))
}

/// `extractBits()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#extractBits-builtin>
pub fn extractBits(e: &Type, offset: &Type, count: &Type) -> Result<Type, E> {
    if !inner_is_integer(e) {
        Err(E::Builtin(
            "`extractBits` 1st argument must be an integer scalar or vector",
        ))
    } else if offset.is_convertible_to(&Type::U32) && count.is_convertible_to(&Type::U32) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`extractBits` 2nd and 3rd arguments must be u32",
        ))
    }
}

/// `faceForward()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#faceForward-builtin>
pub fn faceForward(e1: &Type, e2: &Type, e3: &Type) -> Result<Type, E> {
    let ty = convert_all_ty([e1, e2, e3])
        .ok_or(E::Builtin("`faceForward` arguments are incompatible"))?;
    if matches!(ty, Type::Vec(_, t) if t.is_float()) {
        Ok(ty.clone())
    } else {
        Err(E::Builtin(
            "`faceForward` expects three float vector arguments",
        ))
    }
}

/// `firstLeadingBit()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#firstLeadingBit-builtin>
pub fn firstLeadingBit(e: &Type) -> Result<Type, E> {
    if inner_is_integer(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`firstLeadingBit` expects an integer scalar or vector argument",
        ))
    }
}

/// `firstTrailingBit()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#firstTrailingBit-builtin>
pub fn firstTrailingBit(e: &Type) -> Result<Type, E> {
    if inner_is_integer(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`firstTrailingBit` expects an integer scalar or vector argument",
        ))
    }
}

/// `floor()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#floor-builtin>
pub fn floor(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`floor` argument must be a float scalar or vector",
    ))
}

/// `fma()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#fma-builtin>
pub fn fma(e1: &Type, e2: &Type, e3: &Type) -> Result<Type, E> {
    let ty = convert_all_ty([e1, e2, e3]).ok_or(E::Builtin("`fma` arguments are incompatible"))?;
    inner_is_float(ty).then_some(ty.clone()).ok_or(E::Builtin(
        "`fma` expects three float scalar or vector arguments",
    ))
}

/// `fract()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#fract-builtin>
pub fn fract(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`fract` argument must be a float scalar or vector",
    ))
}

/// `frexp()` builtin function.
///
/// TODO: This built-in is only partially implemented.
///
/// Reference: <https://www.w3.org/TR/WGSL/#frexp-builtin>
pub fn frexp(e: &Type) -> Result<Type, E> {
    if inner_is_float(e) {
        Ok(frexp_struct_type(e).unwrap().into())
    } else {
        Err(E::Builtin(
            "`frexp` expects a float scalar or vector argument",
        ))
    }
}

/// `insertBits()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#insertBits-builtin>
pub fn insertBits(e: &Type, newbits: &Type, offset: &Type, count: &Type) -> Result<Type, E> {
    let ty = convert_ty(e, newbits).ok_or(E::Builtin("`insertBits` arguments are incompatible"))?;

    if !inner_is_integer(ty) {
        Err(E::Builtin(
            "`insertBits` 1st argument must be an integer scalar or vector",
        ))
    } else if !offset.is_convertible_to(&Type::U32) || !count.is_convertible_to(&Type::U32) {
        Err(E::Builtin("`insertBits` 3rd and 4th arguments must be u32"))
    } else {
        Ok(ty.concretize())
    }
}

/// `inverseSqrt()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#inverseSqrt-builtin>
pub fn inverseSqrt(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`inverseSqrt` argument must be a float scalar or vector",
    ))
}

/// `ldexp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#ldexp-builtin>
pub fn ldexp(e1: &Type, e2: &Type) -> Result<Type, E> {
    if !inner_is_float(e1) {
        Err(E::Builtin(
            "`ldexp` 1st argument must be a float scalar or vector",
        ))
    } else if !e2.inner_ty().is_signed() || !e2.inner_ty().is_integer() {
        Err(E::Builtin(
            "`ldexp` 2nd argument must be a signed integer scalar or vector",
        ))
    } else if matches!((e1, e2), (Type::Vec(n1, _), Type::Vec(n2, _)) if n1 != n2) {
        Err(E::Builtin(
            "`ldexp` vector arguments must have the same number of components",
        ))
    } else if e1.is_vec() && !e2.is_vec() || !e1.is_vec() && e2.is_vec() {
        Err(E::Builtin(
            "`ldexp` arguments must be both scalar or both vectors",
        ))
    } else if e1.is_abstract() && e2.is_concrete() {
        // "If either parameter is concrete then the other parameter will undergo automatic conversion to a concrete type (if applicable) and the result will be a concrete type."
        Ok(e1.concretize())
    } else {
        Ok(e1.clone())
    }
}

/// `length()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#length-builtin>
pub fn length(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.inner_ty()).ok_or(E::Builtin(
        "`length` argument must be a float scalar or vector",
    ))
}

/// `log()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#log-builtin>
pub fn log(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`log` argument must be a float scalar or vector",
    ))
}

/// `log2()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#log2-builtin>
pub fn log2(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`log2` expects a float scalar or vector argument",
    ))
}

/// `max()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#max-builtin>
pub fn max(e1: &Type, e2: &Type) -> Result<Type, E> {
    let ty = convert_ty(e1, e2).ok_or(E::Builtin("`max` arguments are incompatible"))?;
    inner_is_numeric(ty).then_some(ty.clone()).ok_or(E::Builtin(
        "`max` expects two numeric scalar or vector arguments",
    ))
}

/// `min()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#min-builtin>
pub fn min(e1: &Type, e2: &Type) -> Result<Type, E> {
    let ty = convert_ty(e1, e2).ok_or(E::Builtin("`min` arguments are incompatible"))?;
    inner_is_numeric(ty).then_some(ty.clone()).ok_or(E::Builtin(
        "`min` expects two numeric scalar or vector arguments",
    ))
}

/// `mix()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#mix-builtin>
pub fn mix(e1: &Type, e2: &Type, e3: &Type) -> Result<Type, E> {
    // e1 and e2 have to be the same type, but e3 can be the same type or the same inner type.
    let ty =
        convert_ty(e1, e2).ok_or(E::Builtin("`mix` 1st and 2nd arguments are incompatible"))?;

    if !inner_is_float(ty) {
        Err(E::Builtin(
            "`mix` expects three float scalar or vector arguments",
        ))
    }
    // 2nd overload: scalar blend factor with vector mixing components
    else if ty.is_vec() && e3.is_scalar() {
        let ty = convert_ty(&ty.inner_ty(), e3)
            .and_then(|inner_ty| ty.convert_inner_to(inner_ty))
            .ok_or(E::Builtin(
                "`mix` 3rd argument is incompatible with 1st and 2nd argument inner type",
            ))?;
        Ok(ty)
    }
    // 1st overload: 3 args of the same type
    else {
        let ty = convert_ty(ty, e3).ok_or(E::Builtin(
            "`mix` 3rd argument is incompatible with 1st and 2nd arguments",
        ))?;
        Ok(ty.clone())
    }
}

/// `modf()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#modf-builtin>
pub fn modf(e: &Type) -> Result<Type, E> {
    if inner_is_float(e) {
        Ok(modf_struct_type(e).unwrap().into())
    } else {
        Err(E::Builtin(
            "`modf` expects a float scalar or vector argument",
        ))
    }
}

/// `normalize()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#normalize-builtin>
pub fn normalize(e: &Type) -> Result<Type, E> {
    match e {
        // abstractInt is convertible to float
        Type::Vec(n, t) if t.is_abstract_int() => Ok(Type::Vec(*n, Type::AbstractFloat.into())),
        Type::Vec(_, t) if t.is_float() => Ok(e.clone()),
        _ => Err(E::Builtin("`normalize` expects a float vector argument")),
    }
}

/// `pow()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pow-builtin>
pub fn pow(e1: &Type, e2: &Type) -> Result<Type, E> {
    let ty = convert_ty(e1, e2).ok_or(E::Builtin("`pow` arguments are incompatible"))?;
    inner_is_float(ty).then_some(ty.clone()).ok_or(E::Builtin(
        "`pow` argument must be a float scalar or vector",
    ))
}

/// `quantizeToF16()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#quantizeToF16-builtin>
pub fn quantizeToF16(e: &Type) -> Result<Type, E> {
    const ERR: E = E::Builtin("`quantizeToF16` expects a f32 scalar or vector argument");
    let ty = e.convert_inner_to(&Type::F32).ok_or(ERR)?;
    if ty.is_f32() || ty.is_vec() {
        Ok(ty)
    } else {
        Err(ERR)
    }
}

/// `radians()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#radians-builtin>
pub fn radians(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`radians` argument must be a float scalar or vector",
    ))
}

/// `reflect()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#reflect-builtin>
pub fn reflect(e1: &Type, e2: &Type) -> Result<Type, E> {
    let ty = convert_ty(e1, e2).ok_or(E::Builtin("`reflect` arguments are incompatible"))?;
    if ty.is_vec() && ty.inner_ty().is_float() {
        Ok(ty.clone())
    } else {
        Err(E::Builtin("`reflect` expects two float vector arguments"))
    }
}

/// `refract()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#refract-builtin>
pub fn refract(e1: &Type, e2: &Type, e3: &Type) -> Result<Type, E> {
    let ty = convert_ty(e1, e2).ok_or(E::Builtin(
        "`refract` 1st and 2nd arguments are incompatible",
    ))?;

    let ty = convert_ty(&ty.inner_ty(), e3)
        .and_then(|inner_ty| ty.convert_inner_to(inner_ty))
        .ok_or(E::Builtin(
            "`refract` 3rd argument is incompatible with 1st and 2nd argument inner type",
        ))?;

    if ty.is_vec() && ty.inner_ty().is_float() {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`refract` expects two scalar vector arguments and one scalar argument",
        ))
    }
}

/// `reverseBits()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#reverseBits-builtin>
pub fn reverseBits(e: &Type) -> Result<Type, E> {
    if inner_is_integer(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`reverseBits` expects an integer scalar or vector argument",
        ))
    }
}

/// `round()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#round-builtin>
pub fn round(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`round` argument must be a float scalar or vector",
    ))
}

/// `saturate()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#saturate-builtin>
pub fn saturate(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`saturate` argument must be a float scalar or vector",
    ))
}

/// `sign()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sign-builtin>
pub fn sign(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) && e.inner_ty().is_signed() {
        Ok(e.clone())
    } else {
        Err(E::Builtin(
            "`sign` argument must be a signed numeric scalar or vector",
        ))
    }
}

/// `sin()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sin-builtin>
pub fn sin(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`sin` argument must be a float scalar or vector",
    ))
}

/// `sinh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sinh-builtin>
pub fn sinh(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`sinh` argument must be a float scalar or vector",
    ))
}

/// `smoothstep()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#smoothstep-builtin>
pub fn smoothstep(edge0: &Type, edge1: &Type, x: &Type) -> Result<Type, E> {
    let ty = convert_all_ty([edge0, edge1, x])
        .ok_or(E::Builtin("`smoothstep` arguments are incompatible"))?;
    inner_is_float(ty).then_some(ty.clone()).ok_or(E::Builtin(
        "`smoothstep` expects three float scalar or vector arguments",
    ))
}

/// `sqrt()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sqrt-builtin>
pub fn sqrt(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`sqrt` argument must be a float scalar or vector",
    ))
}

/// `step()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#step-builtin>
pub fn step(edge: &Type, x: &Type) -> Result<Type, E> {
    let ty = convert_all_ty([edge, x]).ok_or(E::Builtin("`step` arguments are incompatible"))?;
    inner_is_float(ty).then_some(ty.clone()).ok_or(E::Builtin(
        "`step` expects two float scalar or vector arguments",
    ))
}

/// `tan()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#tan-builtin>
pub fn tan(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`tan` argument must be a float scalar or vector",
    ))
}

/// `tanh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#tanh-builtin>
pub fn tanh(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`tanh` argument must be a float scalar or vector",
    ))
}

/// `transpose()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#transpose-builtin>
pub fn transpose(e: &Type) -> Result<Type, E> {
    match e {
        Type::Mat(r, c, ty) => Ok(Type::Mat(*c, *r, ty.clone())),
        _ => Err(E::Builtin("`transpose` expects a matrix argument")),
    }
}

/// `trunc()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#trunc-builtin>
pub fn trunc(e: &Type) -> Result<Type, E> {
    inner_is_float(e).then_some(e.clone()).ok_or(E::Builtin(
        "`trunc` argument must be a float scalar or vector",
    ))
}

// ----------
// DERIVATIVE
// ----------
// reference: <https://www.w3.org/TR/WGSL/#derivative-builtin-functions>

/// `dpdx()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dpdx-builtin>
pub fn dpdx(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`dpdx` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `dpdxCoarse()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dpdxCoarse-builtin>
pub fn dpdxCoarse(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`dpdxCoarse` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `dpdxFine()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dpdxFine-builtin>
pub fn dpdxFine(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`dpdxFine` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `dpdy()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dpdy-builtin>
pub fn dpdy(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`dpdy` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `dpdyCoarse()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dpdyCoarse-builtin>
pub fn dpdyCoarse(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`dpdyCoarse` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `dpdyFine()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dpdyFine-builtin>
pub fn dpdyFine(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`dpdyFine` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `fwidth()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#fwidth-builtin>
pub fn fwidth(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`fwidth` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `fwidthCoarse()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#fwidthCoarse-builtin>
pub fn fwidthCoarse(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`fwidthCoarse` expects a `f32` scalar or vector argument",
        ))
    }
}

/// `fwidthFine()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#fwidthFine-builtin>
pub fn fwidthFine(e: &Type) -> Result<Type, E> {
    let ty = e.convert_inner_to(&Type::F32);
    if let Some(ty) = ty
        && (ty.is_scalar() || ty.is_vec())
    {
        Ok(ty)
    } else {
        Err(E::Builtin(
            "`fwidthFine` expects a `f32` scalar or vector argument",
        ))
    }
}

// -------
// TEXTURE
// -------
// reference: <https://www.w3.org/TR/WGSL/#texture-builtin-functions>

/// `textureDimensions()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureDimensions>
pub fn textureDimensions(t: &Type, level: Option<&Type>) -> Result<Type, E> {
    match t {
        Type::Texture(t) => {
            let ret = match t.dimensions() {
                TextureDimensions::D1 => Type::U32,
                TextureDimensions::D2 => Type::Vec(2, Type::U32.into()),
                TextureDimensions::D3 => Type::Vec(3, Type::U32.into()),
            };

            if let Some(l) = level
                && !l.is_integer()
            {
                Err(E::Builtin(
                    "`textureDimensions` 2nd argument must be an integer",
                ))
            } else {
                Ok(ret)
            }
        }
        _ => Err(E::Builtin(
            "`textureDimensions` 1st argument must be a texture",
        )),
    }
}

/// `textureGather()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureGather>
// TODO: typecheck the other arguments
pub fn textureGather(
    e1: &Type,
    e2: &Type,
    _e3: &Type,
    _e4: Option<&Type>,
    _e5: Option<&Type>,
    _e6: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1 {
        if t.is_depth() {
            Ok(Type::Vec(4, Type::F32.into()))
        } else {
            Err(E::Builtin(
                "the first argument to `textureGather` must be either a depth texture or an integer",
            ))
        }
    } else if let Type::Texture(t) = e2 {
        if let Some(sample_type) = t.sampled_type() {
            Ok(Type::Vec(4, Box::new(sample_type.into())))
        } else {
            Err(E::Builtin(
                "the 2nd argument to `textureGather` must be a sampled texture when the 1st one is an integer",
            ))
        }
    } else {
        Err(E::Builtin(
            "`textureGather` expects a texture in the 1st or 2nd argument",
        ))
    }
}

/// `textureGatherCompare()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureGatherCompare>
// TODO: typecheck the other arguments
pub fn textureGatherCompare(
    e1: &Type,
    _e2: &Type,
    _e3: &Type,
    _e4: &Type,
    _e5: Option<&Type>,
    _e6: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1
        && t.is_depth()
    {
        Ok(Type::Vec(4, Type::F32.into()))
    } else {
        Err(E::Builtin(
            "`textureGatherCompare` 1st argument must be a depth texture",
        ))
    }
}

/// `textureLoad()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureLoad>
// TODO: typecheck the other arguments
pub fn textureLoad(
    e1: &Type,
    _e2: &Type,
    _e3: Option<&Type>,
    _e4: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1 {
        if t.is_cube() {
            Err(E::Builtin("`textureLoad` does not support cube textures"))
        } else if t.is_depth() || *t == TextureType::DepthMultisampled2D {
            // NOTE: a `texture_depth_multisampled_2d` is *not* considered a depth texture.
            Ok(Type::F32)
        } else {
            Ok(Type::Vec(4, Box::new(t.channel_type().into())))
        }
    } else {
        Err(E::Builtin("`textureLoad` 1st argument must be a texture"))
    }
}

/// `textureNumLayers()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureNumLayers>
pub fn textureNumLayers(t: &Type) -> Result<Type, E> {
    if let Type::Texture(t) = t
        && t.is_arrayed()
    {
        Ok(Type::U32)
    } else {
        Err(E::Builtin(
            "`textureNumLayers` expects an array texture argument",
        ))
    }
}

/// `textureNumLevels()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureNumLevels>
pub fn textureNumLevels(t: &Type) -> Result<Type, E> {
    if let Type::Texture(t) = t
        && (t.is_sampled() || t.is_depth())
    {
        Ok(Type::U32)
    } else {
        Err(E::Builtin(
            "`textureNumLevels` expects an sampled or depth texture argument",
        ))
    }
}

/// `textureNumSamples()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureNumSamples>
pub fn textureNumSamples(t: &Type) -> Result<Type, E> {
    if let Type::Texture(t) = t
        && t.is_multisampled()
    {
        Ok(Type::U32)
    } else {
        Err(E::Builtin(
            "`textureNumSamples` expects a multisampled texture argument",
        ))
    }
}

/// `textureSample()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureSample>
// TODO: typecheck the other arguments
pub fn textureSample(
    e1: &Type,
    _e2: &Type,
    _e3: &Type,
    _e4: Option<&Type>,
    _e5: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1 {
        if t.is_sampled() {
            Ok(Type::Vec(4, Type::F32.into()))
        } else if t.is_depth() {
            Ok(Type::F32)
        } else {
            Err(E::Builtin(
                "`textureSample` first argument must be a sampled or depth texture",
            ))
        }
    } else {
        Err(E::Builtin(
            "`textureSample` first argument must be a sampled or depth texture",
        ))
    }
}

/// `textureSampleBias()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureSampleBias>
// TODO: typecheck the other arguments
pub fn textureSampleBias(
    e1: &Type,
    _e2: &Type,
    _e3: &Type,
    _e4: &Type,
    _e5: Option<&Type>,
    _e6: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1 {
        if t.dimensions() == TextureDimensions::D1 {
            Err(E::Builtin(
                "`textureSampleBias` texture cannot be 1-dimensional",
            ))
        } else if t.is_sampled() {
            Ok(Type::Vec(4, Type::F32.into()))
        } else {
            Err(E::Builtin(
                "`textureSampleBias` first argument must be a sampled texture",
            ))
        }
    } else {
        Err(E::Builtin(
            "`textureSampleBias` first argument must be a sampled texture",
        ))
    }
}

/// `textureSampleCompare()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureSampleCompare>
// TODO: typecheck the other arguments
pub fn textureSampleCompare(
    e1: &Type,
    _e2: &Type,
    _e3: &Type,
    _e4: &Type,
    _e5: Option<&Type>,
    _e6: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1
        && t.is_depth()
    {
        Ok(Type::F32)
    } else {
        Err(E::Builtin(
            "`textureSampleCompare` first argument must be a depth texture",
        ))
    }
}

/// `textureSampleCompareLevel()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureSampleCompareLevel>
// TODO: typecheck the other arguments
pub fn textureSampleCompareLevel(
    e1: &Type,
    _e2: &Type,
    _e3: &Type,
    _e4: &Type,
    _e5: Option<&Type>,
    _e6: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1
        && t.is_depth()
    {
        Ok(Type::F32)
    } else {
        Err(E::Builtin(
            "`textureSampleCompareLevel` first argument must be a depth texture",
        ))
    }
}

/// `textureSampleGrad()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureSampleGrad>
// TODO: typecheck the other arguments
pub fn textureSampleGrad(
    e1: &Type,
    _e2: &Type,
    _e3: &Type,
    _e4: &Type,
    _e5: &Type,
    _e6: Option<&Type>,
    _e7: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1 {
        if t.dimensions() == TextureDimensions::D1 {
            Err(E::Builtin(
                "`textureSampleGrad` texture cannot be 1-dimensional",
            ))
        } else if t.is_sampled() {
            Ok(Type::Vec(4, Type::F32.into()))
        } else {
            Err(E::Builtin(
                "`textureSampleGrad` first argument must be a sampled texture",
            ))
        }
    } else {
        Err(E::Builtin(
            "`textureSampleGrad` first argument must be a sampled texture",
        ))
    }
}

/// `textureSampleLevel()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureSampleLevel>
// TODO: typecheck the other arguments
pub fn textureSampleLevel(
    e1: &Type,
    _e2: &Type,
    _e3: &Type,
    _e4: &Type,
    _e5: Option<&Type>,
    _e6: Option<&Type>,
) -> Result<Type, E> {
    if let Type::Texture(t) = e1 {
        if t.is_sampled() {
            Ok(Type::Vec(4, Type::F32.into()))
        } else if t.is_depth() {
            Ok(Type::F32)
        } else {
            Err(E::Builtin(
                "`textureSampleLevel` first argument must be a sampled or depth texture",
            ))
        }
    } else {
        Err(E::Builtin(
            "`textureSampleLevel` first argument must be a sampled or depth texture",
        ))
    }
}

/// `textureSampleBaseClampToEdge()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureSampleBaseClampToEdge>
pub fn textureSampleBaseClampToEdge(e1: &Type, _e2: &Type, _e3: &Type) -> Result<Type, E> {
    if matches!(
        e1,
        Type::Texture(TextureType::Sampled2D(SampledType::F32) | TextureType::External)
    ) {
        Ok(Type::Vec(4, Type::F32.into()))
    } else {
        Err(E::Builtin(
            "`textureSampleBaseClampToEdge` first argument must be a `texture_2d<f32> ` or `texture_external`",
        ))
    }
}

/// `textureStore()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#textureStore>
// TODO: typecheck the other arguments
pub fn textureStore(e1: &Type, _e2: &Type, _e3: &Type, _e4: Option<&Type>) -> Result<(), E> {
    if let Type::Texture(t) = e1
        && t.is_storage()
    {
        Ok(())
    } else {
        Err(E::Builtin(
            "`textureStore` first argument must be a storage texture",
        ))
    }
}

// ------
// ATOMIC
// ------
// reference: <https://www.w3.org/TR/WGSL/#atomic-builtin-functions>

/// `atomicLoad()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicLoad-builtin>
pub fn atomicLoad(e: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else {
            Ok(*ty.clone())
        }
    } else {
        Err(E::Builtin(
            "`atomicLoad` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicStore()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicStore-builtin>
pub fn atomicStore(e1: &Type, e2: &Type) -> Result<(), E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(())
        } else {
            Err(E::Builtin(
                "`atomicStore` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicStore` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicAdd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicAdd-builtin>
pub fn atomicAdd(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicAdd` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicAdd` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicSub()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicSub-builtin>
pub fn atomicSub(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicSub` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicSub` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicMax()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicMax-builtin>
pub fn atomicMax(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicMax` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicMax` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicMin()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicMin-builtin>
pub fn atomicMin(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicMin` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicMin` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicAnd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicAnd-builtin>
pub fn atomicAnd(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicAnd` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicAnd` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicOr()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicOr-builtin>
pub fn atomicOr(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicOr` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicOr` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicXor()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicXor-builtin>
pub fn atomicXor(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicXor` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicXor` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicExchange()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicExchange-builtin>
pub fn atomicExchange(e1: &Type, e2: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) {
            Ok(*ty.clone())
        } else {
            Err(E::Builtin(
                "`atomicExchange` 2nd argument is incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicExchange` expects a pointer to atomic argument",
        ))
    }
}

/// `atomicCompareExchangeWeak()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicCompareExchangeWeak-builtin>
pub fn atomicCompareExchangeWeak(e1: &Type, e2: &Type, e3: &Type) -> Result<Type, E> {
    if let Type::Ptr(a_s, ptr_ty, a_m) = e1
        && let Type::Atomic(ty) = &**ptr_ty
    {
        if *a_s != AddressSpace::Storage && *a_s != AddressSpace::Workgroup {
            Err(E::Builtin(
                "the address space of the atomic pointer argument must be `storage` or `workgroup`",
            ))
        } else if *a_m != AccessMode::ReadWrite {
            Err(E::Builtin(
                "the access mode of the atomic pointer argument must be `read_write`",
            ))
        } else if e2.is_convertible_to(ty) && e3.is_convertible_to(ty) {
            Ok(atomic_compare_exchange_struct_type(ty).into())
        } else {
            Err(E::Builtin(
                "`atomicCompareExchangeWeak` 2nd and 3rd arguments are incompatible with the atomic pointer type",
            ))
        }
    } else {
        Err(E::Builtin(
            "`atomicCompareExchangeWeak` expects a pointer to atomic argument",
        ))
    }
}

// ------------
// DATA PACKING
// ------------
// reference: <https://www.w3.org/TR/WGSL/#pack-builtin-functions>

/// `pack4x8snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4x8snorm-builtin>
pub fn pack4x8snorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(4, Type::F32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack4x8snorm` expects a `vec4<f32>` argument"))
    }
}

/// `pack4x8unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4x8unorm-builtin>
pub fn pack4x8unorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(4, Type::F32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack4x8unorm` expects a `vec4<f32>` argument"))
    }
}

/// `pack4xI8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xI8-builtin>
pub fn pack4xI8(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(4, Type::I32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack4xI8` expects a `vec4<i32>` argument"))
    }
}

/// `pack4xU8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xU8-builtin>
pub fn pack4xU8(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(4, Type::U32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack4xU8` expects a `vec4<u32>` argument"))
    }
}

/// `pack4xI8Clamp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xI8Clamp-builtin>
pub fn pack4xI8Clamp(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(4, Type::I32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack4xI8Clamp` expects a `vec4<i32>` argument"))
    }
}

/// `pack4xU8Clamp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xU8Clamp-builtin>
pub fn pack4xU8Clamp(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(4, Type::U32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack4xU8Clamp` expects a `vec4<u32>` argument"))
    }
}

/// `pack2x16snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack2x16snorm-builtin>
pub fn pack2x16snorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(2, Type::F32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack2x16snorm` expects a `vec2<f32>` argument"))
    }
}

/// `pack2x16unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack2x16unorm-builtin>
pub fn pack2x16unorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(2, Type::F32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack2x16unorm` expects a `vec2<f32>` argument"))
    }
}

/// `pack2x16float()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack2x16float-builtin>
pub fn pack2x16float(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::Vec(2, Type::F32.into())) {
        Ok(Type::U32)
    } else {
        Err(E::Builtin("`pack2x16float` expects a `vec2<f32>` argument"))
    }
}

/// `unpack4x8snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4x8snorm-builtin>
pub fn unpack4x8snorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::U32) {
        Ok(Type::Vec(4, Type::F32.into()))
    } else {
        Err(E::Builtin("`unpack4x8snorm` expects a `u32` argument"))
    }
}

/// `unpack4x8unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4x8unorm-builtin>
pub fn unpack4x8unorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::U32) {
        Ok(Type::Vec(4, Type::F32.into()))
    } else {
        Err(E::Builtin("`unpack4x8unorm` expects a `u32` argument"))
    }
}

/// `unpack4xI8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4xI8-builtin>
pub fn unpack4xI8(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::U32) {
        Ok(Type::Vec(4, Type::I32.into()))
    } else {
        Err(E::Builtin("`unpack4xI8` expects a `u32` argument"))
    }
}

/// `unpack4xU8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4xU8-builtin>
pub fn unpack4xU8(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::U32) {
        Ok(Type::Vec(4, Type::U32.into()))
    } else {
        Err(E::Builtin("`unpack4xU8` expects a `u32` argument"))
    }
}

/// `unpack2x16snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack2x16snorm-builtin>
pub fn unpack2x16snorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::U32) {
        Ok(Type::Vec(2, Type::F32.into()))
    } else {
        Err(E::Builtin("`unpack2x16snorm` expects a `u32` argument"))
    }
}

/// `unpack2x16unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack2x16unorm-builtin>
pub fn unpack2x16unorm(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::U32) {
        Ok(Type::Vec(2, Type::F32.into()))
    } else {
        Err(E::Builtin("`unpack2x16unorm` expects a `u32` argument"))
    }
}

/// `unpack2x16float()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack2x16float-builtin>
pub fn unpack2x16float(e: &Type) -> Result<Type, E> {
    if e.is_convertible_to(&Type::U32) {
        Ok(Type::Vec(2, Type::F32.into()))
    } else {
        Err(E::Builtin("`unpack2x16float` expects a `u32` argument"))
    }
}

// --------
// SUBGROUP
// --------
// reference: <https://www.w3.org/TR/WGSL/#subgroup-builtin-functions>

/// `subgroupAdd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupAdd-builtin>
pub fn subgroupAdd(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupAdd` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupExclusiveAdd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupExclusiveAdd-builtin>
pub fn subgroupExclusiveAdd(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupExclusiveAdd` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupInclusiveAdd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupInclusiveAdd-builtin>
pub fn subgroupInclusiveAdd(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupInclusiveAdd` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupAll()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupAll-builtin>
pub fn subgroupAll(e: &Type) -> Result<Type, E> {
    if e.is_bool() {
        Ok(Type::Bool)
    } else {
        Err(E::Builtin("`subgroupAll` expects a boolean argument"))
    }
}

/// `subgroupAnd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupAnd-builtin>
pub fn subgroupAnd(e: &Type) -> Result<Type, E> {
    if inner_is_integer(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupAnd` expects an integer scalar or vector argument",
        ))
    }
}

/// `subgroupAny()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupAny-builtin>
pub fn subgroupAny(e: &Type) -> Result<Type, E> {
    if e.is_bool() {
        Ok(Type::Bool)
    } else {
        Err(E::Builtin("`subgroupAny` expects a boolean argument"))
    }
}

/// `subgroupBallot()` builtin function.
///
/// NOTE: The `naga-ext` extension allows omitting the predicate argument.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupBallot-builtin>
pub fn subgroupBallot(pred: Option<&Type>) -> Result<Type, E> {
    if let Some(pred) = pred
        && pred.is_bool()
    {
        Ok(Type::Vec(4, Type::U32.into()))
    } else if pred == None && cfg!(feature = "naga-ext") {
        Ok(Type::Vec(4, Type::U32.into()))
    } else {
        Err(E::Builtin("`subgroupBallot` expects a boolean argument"))
    }
}

/// `subgroupBroadcast()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupBroadcast-builtin>
pub fn subgroupBroadcast(e: &Type, id: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) && id.is_integer() {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupBroadcast` expects a numeric scalar or vector 1st argument and an integer 2nd argument",
        ))
    }
}

/// `subgroupBroadcastFirst()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupBroadcastFirst-builtin>
pub fn subgroupBroadcastFirst(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupBroadcastFirst` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupElect()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupElect-builtin>
pub fn subgroupElect() -> Result<Type, E> {
    Ok(Type::Bool)
}

/// `subgroupMax()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupMax-builtin>
pub fn subgroupMax(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupMax` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupMin()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupMin-builtin>
pub fn subgroupMin(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupMin` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupMul()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupMul-builtin>
pub fn subgroupMul(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupMul` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupExclusiveMul()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupExclusiveMul-builtin>
pub fn subgroupExclusiveMul(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupExclusiveMul` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupInclusiveMul()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupInclusiveMul-builtin>
pub fn subgroupInclusiveMul(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupInclusiveMul` expects a numeric scalar or vector argument",
        ))
    }
}

/// `subgroupOr()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupOr-builtin>
pub fn subgroupOr(e: &Type) -> Result<Type, E> {
    if inner_is_integer(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupOr` expects an integer scalar or vector argument",
        ))
    }
}

/// `subgroupShuffle()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupShuffle-builtin>
pub fn subgroupShuffle(e: &Type, id: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) && id.is_integer() {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupShuffle` expects a numeric scalar or vector 1st argument and an integer 2nd argument",
        ))
    }
}

/// `subgroupShuffleDown()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupShuffleDown-builtin>
pub fn subgroupShuffleDown(e: &Type, delta: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) && delta.is_convertible_to(&Type::U32) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupShuffleDown` expects a numeric scalar or vector 1st argument and an integer 2nd argument",
        ))
    }
}

/// `subgroupShuffleUp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupShuffleUp-builtin>
pub fn subgroupShuffleUp(e: &Type, delta: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) && delta.is_convertible_to(&Type::U32) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupShuffleUp` expects a numeric scalar or vector 1st argument and an integer 2nd argument",
        ))
    }
}

/// `subgroupShuffleXor()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupShuffleXor-builtin>
pub fn subgroupShuffleXor(e: &Type, mask: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) && mask.is_convertible_to(&Type::U32) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupShuffleXor` expects a numeric scalar or vector 1st argument and an integer 2nd argument",
        ))
    }
}

/// `subgroupXor()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#subgroupXor-builtin>
pub fn subgroupXor(e: &Type) -> Result<Type, E> {
    if inner_is_integer(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`subgroupXor` expects an integer scalar or vector argument",
        ))
    }
}

// ----
// QUAD
// ----
// reference: <https://www.w3.org/TR/WGSL/#quad-builtin-functions>

/// `quadBroadcast()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#quadBroadcast-builtin>
pub fn quadBroadcast(e: &Type, id: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) && id.is_integer() {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`quadBroadcast` expects a numeric scalar or vector 1st argument and an integer 2nd argument",
        ))
    }
}

/// `quadSwapDiagonal()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#quadSwapDiagonal-builtin>
pub fn quadSwapDiagonal(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`quadSwapDiagonal` expects a numeric scalar or vector argument",
        ))
    }
}

/// `quadSwapX()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#quadSwapX-builtin>
pub fn quadSwapX(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`quadSwapX` expects a numeric scalar or vector argument",
        ))
    }
}

/// `quadSwapY()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#quadSwapY-builtin>
pub fn quadSwapY(e: &Type) -> Result<Type, E> {
    if inner_is_numeric(e) {
        Ok(e.concretize())
    } else {
        Err(E::Builtin(
            "`quadSwapY` expects a numeric scalar or vector argument",
        ))
    }
}

// -----------------------
// NAGA RAY QUERY EXTENSION
// -----------------------
// These built-ins are `naga` extensions and are not part of the WGSL specification.
//
// TODO: validate naga extensions arguments

/// `rayQueryInitialize()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn rayQueryInitialize(rq: &Type, accel_struct: &Type, ray_desc: &Type) -> Result<(), E> {
    if matches!(
        rq,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) && matches!(accel_struct, Type::AccelerationStructure(_))
        && matches!(ray_desc, Type::Struct(s) if s.name == "RayDesc")
    {
        Ok(())
    } else {
        Err(E::Builtin(
            "`rayQueryInitialize` expects a pointer to `ray_query`, an acceleration structure and a `RayDesc` argument",
        ))
    }
}

/// `rayQueryProceed()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn rayQueryProceed(rq: &Type) -> Result<Type, E> {
    if matches!(
        rq,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Ok(Type::Bool)
    } else {
        Err(E::Builtin(
            "`rayQueryProceed` expects a pointer to `ray_query` argument",
        ))
    }
}

/// `rayQueryGenerateIntersection()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn rayQueryGenerateIntersection(rq: &Type, hit_t: &Type) -> Result<(), E> {
    if !matches!(
        rq,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Err(E::Builtin(
            "`rayQueryGenerateIntersection` 1st argument must be a pointer to `ray_query`",
        ))
    } else if hit_t.is_convertible_to(&Type::F32) {
        Ok(())
    } else {
        Err(E::Builtin(
            "`rayQueryGenerateIntersection` 2nd argument must be a `f32`",
        ))
    }
}

/// `rayQueryConfirmIntersection()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn rayQueryConfirmIntersection(rq: &Type) -> Result<(), E> {
    if !matches!(
        rq,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Err(E::Builtin(
            "`rayQueryConfirmIntersection` expects a pointer to `ray_query` argument",
        ))
    } else {
        Ok(())
    }
}

/// `rayQueryTerminate()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn rayQueryTerminate(rq: &Type) -> Result<(), E> {
    if !matches!(
        rq,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Err(E::Builtin(
            "`rayQueryTerminate` expects a pointer to `ray_query` argument",
        ))
    } else {
        Ok(())
    }
}

/// `rayQueryGetCommittedIntersection()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn rayQueryGetCommittedIntersection(e: &Type) -> Result<Type, E> {
    if matches!(
        e,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Ok(ray_intersection_struct_type().into())
    } else {
        Err(E::Builtin(
            "`rayQueryGetCommittedIntersection` expects a pointer to `ray_query` argument",
        ))
    }
}

/// `rayQueryGetCandidateIntersection()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn rayQueryGetCandidateIntersection(e: &Type) -> Result<Type, E> {
    if matches!(
        e,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Ok(ray_intersection_struct_type().into())
    } else {
        Err(E::Builtin(
            "`rayQueryGetCandidateIntersection` expects a pointer to `ray_query` argument",
        ))
    }
}

/// `getCommittedHitVertexPositions()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn getCommittedHitVertexPositions(e: &Type) -> Result<Type, E> {
    if matches!(
        e,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Ok(Type::Array(
            Box::new(Type::Vec(3, Box::new(Type::F32))),
            Some(3),
        ))
    } else {
        Err(E::Builtin(
            "`getCommittedHitVertexPositions` expects a pointer to `ray_query` argument",
        ))
    }
}

/// `getCandidateHitVertexPositions()` `naga` built-in function.
#[cfg(feature = "naga-ext")]
pub fn getCandidateHitVertexPositions(e: &Type) -> Result<Type, E> {
    if matches!(
        e,
        Type::Ptr(AddressSpace::Function, t, AccessMode::ReadWrite) if matches!(**t, Type::RayQuery(_))
    ) {
        Ok(Type::Array(
            Box::new(Type::Vec(3, Box::new(Type::F32))),
            Some(3),
        ))
    } else {
        Err(E::Builtin(
            "`getCandidateHitVertexPositions` expects a pointer to `ray_query` argument",
        ))
    }
}
