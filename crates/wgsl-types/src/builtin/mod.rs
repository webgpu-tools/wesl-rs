//! Implementation and type-checking of built-in functions, constructors and operators.
//!
//! ### Warning
//!
//! Some functions are still TODO and are documented as such.
//!
//! ### Functions
//!
//! See [`call_builtin_fn`]/[`type_builtin_fn`] Individual functions in module [`call`].
//!
//! ### Constructors
//!
//! See [`call_ctor`]/[`type_ctor`]. Individual functions in module [`ctor`].
//!
//! ### Operators
//!
//! See [`call_unary_op`]/[`type_unary_op`] and [`call_binary_op`]/[`type_binary_op`].
//! Individual operators are implemented on the [`Instance`] and [`Type`] types directly
//! with the `op_*` functions.

pub mod call;
pub mod ctor;

mod call_ty;
mod ops;
mod ops_ty;

pub use call_ty::type_builtin_fn;
pub use ctor::{is_ctor, struct_ctor, type_ctor, typecheck_struct_ctor};
pub use ops_ty::{type_binary_op, type_unary_op};

pub(crate) use call_ty::*;
pub(crate) use ops::Compwise;

use itertools::Itertools;

#[cfg(feature = "naga-ext")]
use crate::tplt::AccelerationStructureTags;
use crate::{
    CallSignature, Error, Instance, ShaderStage,
    syntax::{BinaryOperator, UnaryOperator},
    tplt::{
        ArrayTemplate, AtomicTemplate, BitcastTemplate, MatTemplate, PtrTemplate, TextureTemplate,
        TpltParam, VecTemplate,
    },
    ty::{SamplerType, TextureType, Ty, Type},
    ty_context::TyContext,
};

type E = Error;

/// Call a built-in function.
///
/// The arguments must be [loaded][Type::loaded].
///
/// Includes built-in constructors and zero-value constructors, *except* the struct
/// zero-value constructor, since it requires knowledge of the struct type.
/// See [`call_ctor`] or [`Instance::zero_value`] for that.
///
/// Some functions are still TODO, see [`call`] for the list of functions and statuses.
pub fn call_builtin_fn(
    name: &str,
    tplt: Option<&[TpltParam]>,
    args: &[Instance],
    stage: ShaderStage,
    context: &mut TyContext,
) -> Result<Option<Instance>, E> {
    match (name, tplt, args) {
        // constructors
        ("array", Some(t), []) => Instance::zero_value(&ArrayTemplate::parse(t)?.ty(), context),
        ("array", Some(t), a) => {
            let tplt = ArrayTemplate::parse(t)?;
            ctor::array_t(
                &tplt.inner_ty(),
                tplt.n().ok_or(E::TemplateArgs("array"))?,
                a,
                context,
            )
        }
        ("array", None, a) => ctor::array(a, context),
        ("bool", None, []) => Instance::zero_value(&Type::Bool, context),
        ("bool", None, [a1]) => ctor::bool(a1),
        ("i32", None, []) => Instance::zero_value(&Type::I32, context),
        ("i32", None, [a1]) => ctor::i32(a1),
        ("u32", None, []) => Instance::zero_value(&Type::U32, context),
        ("u32", None, [a1]) => ctor::u32(a1),
        ("f32", None, []) => Instance::zero_value(&Type::F32, context),
        ("f32", None, [a1]) => ctor::f32(a1, stage),
        ("f16", None, []) => Instance::zero_value(&Type::F16, context),
        ("f16", None, [a1]) => ctor::f16(a1, stage),
        ("mat2x2", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(2, 2), context),
        ("mat2x2", Some(t), a) => {
            ctor::mat_t(2, 2, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat2x2", None, a) => ctor::mat(2, 2, a, context),
        ("mat2x3", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(2, 3), context),
        ("mat2x3", Some(t), a) => {
            ctor::mat_t(2, 3, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat2x3", None, a) => ctor::mat(2, 3, a, context),
        ("mat2x4", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(2, 4), context),
        ("mat2x4", Some(t), a) => {
            ctor::mat_t(2, 4, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat2x4", None, a) => ctor::mat(2, 4, a, context),
        ("mat3x2", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(3, 2), context),
        ("mat3x2", Some(t), a) => {
            ctor::mat_t(3, 2, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat3x2", None, a) => ctor::mat(3, 2, a, context),
        ("mat3x3", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(3, 3), context),
        ("mat3x3", Some(t), a) => {
            ctor::mat_t(3, 3, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat3x3", None, a) => ctor::mat(3, 3, a, context),
        ("mat3x4", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(3, 4), context),
        ("mat3x4", Some(t), a) => {
            ctor::mat_t(3, 4, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat3x4", None, a) => ctor::mat(3, 4, a, context),
        ("mat4x2", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(4, 2), context),
        ("mat4x2", Some(t), a) => {
            ctor::mat_t(4, 2, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat4x2", None, a) => ctor::mat(4, 2, a, context),
        ("mat4x3", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(4, 3), context),
        ("mat4x3", Some(t), a) => {
            ctor::mat_t(4, 3, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat4x3", None, a) => ctor::mat(4, 3, a, context),
        ("mat4x4", Some(t), []) => Instance::zero_value(&MatTemplate::parse(t)?.ty(4, 4), context),
        ("mat4x4", Some(t), a) => {
            ctor::mat_t(4, 4, MatTemplate::parse(t)?.inner_ty(), a, stage, context)
        }
        ("mat4x4", None, a) => ctor::mat(4, 4, a, context),
        ("vec2", Some(t), []) => {
            Instance::zero_value(&VecTemplate::parse(t, context)?.ty(2), context)
        }
        ("vec2", Some(t), a) => ctor::vec_t(
            2,
            VecTemplate::parse(t, context)?.inner_ty(),
            a,
            stage,
            context,
        ),
        ("vec2", None, a) => ctor::vec(2, a, context),
        ("vec3", Some(t), []) => {
            Instance::zero_value(&VecTemplate::parse(t, context)?.ty(3), context)
        }
        ("vec3", Some(t), a) => ctor::vec_t(
            3,
            VecTemplate::parse(t, context)?.inner_ty(),
            a,
            stage,
            context,
        ),
        ("vec3", None, a) => ctor::vec(3, a, context),
        ("vec4", Some(t), []) => {
            Instance::zero_value(&VecTemplate::parse(t, context)?.ty(4), context)
        }
        ("vec4", Some(t), a) => ctor::vec_t(
            4,
            VecTemplate::parse(t, context)?.inner_ty(),
            a,
            stage,
            context,
        ),
        ("vec4", None, a) => ctor::vec(4, a, context),
        #[cfg(feature = "naga-ext")]
        ("i64", None, []) => Instance::zero_value(&Type::I64, context),
        #[cfg(feature = "naga-ext")]
        ("i64", None, [a1]) => ctor::i64(a1),
        #[cfg(feature = "naga-ext")]
        ("u64", None, []) => Instance::zero_value(&Type::U64, context),
        #[cfg(feature = "naga-ext")]
        ("u64", None, [a1]) => ctor::u64(a1),
        #[cfg(feature = "naga-ext")]
        ("f64", None, []) => Instance::zero_value(&Type::F64, context),
        #[cfg(feature = "naga-ext")]
        ("f64", None, [a1]) => ctor::f64(a1, stage),
        // bitcast
        ("bitcast", Some(t), [a1]) => call::bitcast_t(BitcastTemplate::parse(t)?.ty(), a1, context),
        // logical
        ("all", None, [a]) => call::all(a),
        ("any", None, [a]) => call::any(a),
        ("select", None, [a1, a2, a3]) => call::select(a1, a2, a3, context),
        // array
        ("arrayLength", None, [a]) => call::arrayLength(a, context),
        // numeric
        ("abs", None, [a]) => call::abs(a),
        ("acos", None, [a]) => call::acos(a, context),
        ("acosh", None, [a]) => call::acosh(a, context),
        ("asin", None, [a]) => call::asin(a, context),
        ("asinh", None, [a]) => call::asinh(a, context),
        ("atan", None, [a]) => call::atan(a, context),
        ("atanh", None, [a]) => call::atanh(a, context),
        ("atan2", None, [a1, a2]) => call::atan2(a1, a2, context),
        ("ceil", None, [a]) => call::ceil(a, context),
        ("clamp", None, [a1, a2, a3]) => call::clamp(a1, a2, a3, context),
        ("cos", None, [a]) => call::cos(a, context),
        ("cosh", None, [a]) => call::cosh(a, context),
        ("countLeadingZeros", None, [a]) => call::countLeadingZeros(a, context),
        ("countOneBits", None, [a]) => call::countOneBits(a, context),
        ("countTrailingZeros", None, [a]) => call::countTrailingZeros(a, context),
        ("cross", None, [a1, a2]) => call::cross(a1, a2, stage, context),
        ("degrees", None, [a]) => call::degrees(a, context),
        ("determinant", None, [a]) => call::determinant(a, context),
        ("distance", None, [a1, a2]) => call::distance(a1, a2, stage, context),
        ("dot", None, [a1, a2]) => call::dot(a1, a2, stage, context),
        ("dot4U8Packed", None, [a1, a2]) => call::dot4U8Packed(a1, a2, context),
        ("dot4I8Packed", None, [a1, a2]) => call::dot4I8Packed(a1, a2, context),
        ("exp", None, [a]) => call::exp(a, context),
        ("exp2", None, [a]) => call::exp2(a, context),
        ("extractBits", None, [a1, a2, a3]) => call::extractBits(a1, a2, a3, context),
        ("faceForward", None, [a1, a2, a3]) => call::faceForward(a1, a2, a3, context),
        ("firstLeadingBit", None, [a]) => call::firstLeadingBit(a, context),
        ("firstTrailingBit", None, [a]) => call::firstTrailingBit(a, context),
        ("floor", None, [a]) => call::floor(a, context),
        ("fma", None, [a1, a2, a3]) => call::fma(a1, a2, a3, context),
        ("fract", None, [a]) => call::fract(a, stage, context),
        ("frexp", None, [a]) => call::frexp(a, context),
        ("insertBits", None, [a1, a2, a3, a4]) => call::insertBits(a1, a2, a3, a4, context),
        ("inverseSqrt", None, [a]) => call::inverseSqrt(a, context),
        ("ldexp", None, [a1, a2]) => call::ldexp(a1, a2),
        ("length", None, [a]) => call::length(a, context),
        ("log", None, [a]) => call::log(a, context),
        ("log2", None, [a]) => call::log2(a, context),
        ("max", None, [a1, a2]) => call::max(a1, a2, context),
        ("min", None, [a1, a2]) => call::min(a1, a2, context),
        ("mix", None, [a1, a2, a3]) => call::mix(a1, a2, a3, stage, context),
        ("modf", None, [a]) => call::modf(a, context),
        ("normalize", None, [a]) => call::normalize(a, stage, context),
        ("pow", None, [a1, a2]) => call::pow(a1, a2, context),
        ("quantizeToF16", None, [a]) => call::quantizeToF16(a, context),
        ("radians", None, [a]) => call::radians(a, context),
        ("reflect", None, [a1, a2]) => call::reflect(a1, a2, context),
        ("refract", None, [a1, a2, a3]) => call::refract(a1, a2, a3, context),
        ("reverseBits", None, [a]) => call::reverseBits(a, context),
        ("round", None, [a]) => call::round(a, context),
        ("saturate", None, [a]) => call::saturate(a, context),
        ("sign", None, [a]) => call::sign(a),
        ("sin", None, [a]) => call::sin(a, context),
        ("sinh", None, [a]) => call::sinh(a, context),
        ("smoothstep", None, [a1, a2, a3]) => call::smoothstep(a1, a2, a3, context),
        ("sqrt", None, [a]) => call::sqrt(a, context),
        ("step", None, [a1, a2]) => call::step(a1, a2, context),
        ("tan", None, [a]) => call::tan(a, context),
        ("tanh", None, [a]) => call::tanh(a, context),
        ("transpose", None, [a]) => call::transpose(a),
        ("trunc", None, [a]) => call::trunc(a, context),
        // atomic
        ("atomicLoad", None, [a]) => call::atomicLoad(a, context),
        ("atomicStore", None, [a1, a2]) => {
            call::atomicStore(a1, a2, context)?;
            return Ok(None);
        }
        ("atomicAdd", None, [a1, a2]) => call::atomicAdd(a1, a2, context),
        ("atomicSub", None, [a1, a2]) => call::atomicSub(a1, a2, context),
        ("atomicMax", None, [a1, a2]) => call::atomicMax(a1, a2, context),
        ("atomicMin", None, [a1, a2]) => call::atomicMin(a1, a2, context),
        ("atomicAnd", None, [a1, a2]) => call::atomicAnd(a1, a2, context),
        ("atomicOr", None, [a1, a2]) => call::atomicOr(a1, a2, context),
        ("atomicXor", None, [a1, a2]) => call::atomicXor(a1, a2, context),
        ("atomicExchange", None, [a1, a2]) => call::atomicExchange(a1, a2, context),
        ("atomicCompareExchangeWeak", None, [a1, a2, a3]) => {
            call::atomicCompareExchangeWeak(a1, a2, a3, context)
        }
        // packing
        ("pack4x8snorm", None, [a]) => call::pack4x8snorm(a, context),
        ("pack4x8unorm", None, [a]) => call::pack4x8unorm(a, context),
        ("pack4xI8", None, [a]) => call::pack4xI8(a, context),
        ("pack4xU8", None, [a]) => call::pack4xU8(a, context),
        ("pack4xI8Clamp", None, [a]) => call::pack4xI8Clamp(a, context),
        ("pack4xU8Clamp", None, [a]) => call::pack4xU8Clamp(a, context),
        ("pack2x16snorm", None, [a]) => call::pack2x16snorm(a, context),
        ("pack2x16unorm", None, [a]) => call::pack2x16unorm(a, context),
        ("pack2x16float", None, [a]) => call::pack2x16float(a, context),
        ("unpack4x8snorm", None, [a]) => call::unpack4x8snorm(a, context),
        ("unpack4x8unorm", None, [a]) => call::unpack4x8unorm(a, context),
        ("unpack4xI8", None, [a]) => call::unpack4xI8(a, context),
        ("unpack4xU8", None, [a]) => call::unpack4xU8(a, context),
        ("unpack2x16snorm", None, [a]) => call::unpack2x16snorm(a, context),
        ("unpack2x16unorm", None, [a]) => call::unpack2x16unorm(a, context),
        ("unpack2x16float", None, [a]) => call::unpack2x16float(a, context),
        // synchronization
        // barrier primitives are no-op on the cpu
        ("storageBarrier", None, []) => return Ok(None),
        ("textureBarrier", None, []) => return Ok(None),
        ("workgroupBarrier", None, []) => return Ok(None),
        _ => Err(E::Signature(CallSignature {
            name: name.to_string(),
            tplt: tplt.map(|tplt| tplt.to_vec()),
            args: args.iter().map(|a| a.ty()).collect_vec(),
        })),
    }
    .map(Option::Some)
}

/// Call a constructor.
///
/// The arguments must be [loaded][Type::loaded].
pub fn call_ctor(
    ty: &Type,
    args: &[Instance],
    stage: ShaderStage,
    context: &TyContext,
) -> Result<Instance, E> {
    match (ty, args) {
        (_, []) => Instance::zero_value(ty, context),
        (Type::Bool, [a1]) => ctor::bool(a1),
        (Type::I32, [a1]) => ctor::i32(a1),
        (Type::U32, [a1]) => ctor::u32(a1),
        (Type::F32, [a1]) => ctor::f32(a1, stage),
        (Type::F16, [a1]) => ctor::f16(a1, stage),
        (Type::Struct(ty), a) => struct_ctor(*ty, a, context).map(Instance::from),
        (Type::Array(ty, n), a) => ctor::array_t(ty, n.unwrap_or(a.len()), a, context),
        (Type::Vec(n, ty), a) => ctor::vec_t(*n as usize, ty, a, stage, context),
        (Type::Mat(c, r, ty), a) => ctor::mat_t(*c as usize, *r as usize, ty, a, stage, context),
        (
            Type::AbstractInt
            | Type::AbstractFloat
            | Type::Atomic(_)
            | Type::Ptr(_, _, _)
            | Type::Ref(_, _, _)
            | Type::Texture(_)
            | Type::Sampler(_)
            | Type::Unknown,
            _,
        ) => Err(E::NotConstructible(ty.clone())),
        #[cfg(feature = "naga-ext")]
        (Type::I64 | Type::U64 | Type::F64, _) => Err(E::Todo(
            "naga 64-bit literal constructors not implemented".to_string(),
        )),
        #[cfg(feature = "naga-ext")]
        (Type::BindingArray(_, _), _) => Err(E::NotConstructible(ty.clone())),
        (Type::Bool | Type::I32 | Type::U32 | Type::F32 | Type::F16, _) => {
            Err(E::Signature(CallSignature {
                name: context.display(ty).to_string(),
                tplt: None,
                args: args.iter().map(|a| a.ty()).collect_vec(),
            }))
        }
        #[cfg(feature = "naga-ext")]
        (Type::RayQuery(_) | Type::AccelerationStructure(_), _) => {
            Err(E::NotConstructible(ty.clone()))
        }
    }
}

/// Call a binary operator.
///
/// The arguments must be [loaded][Type::loaded].
///
/// In practice, `&&` and `||` operators are short-circuiting. Calling these operators here
/// does not perform short-circuiting, as `lhs` and `rhs` are already computed.
pub fn call_binary_op(
    op: BinaryOperator,
    lhs: &Instance,
    rhs: &Instance,
    stage: ShaderStage,
    context: &TyContext,
) -> Result<Instance, E> {
    match op {
        BinaryOperator::ShortCircuitOr => lhs.op_or(rhs),
        BinaryOperator::ShortCircuitAnd => lhs.op_and(rhs),
        BinaryOperator::Addition => lhs.op_add(rhs, stage, context),
        BinaryOperator::Subtraction => lhs.op_sub(rhs, stage, context),
        BinaryOperator::Multiplication => lhs.op_mul(rhs, stage, context),
        BinaryOperator::Division => lhs.op_div(rhs, stage, context),
        BinaryOperator::Remainder => lhs.op_rem(rhs, stage, context),
        BinaryOperator::Equality => lhs.op_eq(rhs, context),
        BinaryOperator::Inequality => lhs.op_ne(rhs, context),
        BinaryOperator::LessThan => lhs.op_lt(rhs, context),
        BinaryOperator::LessThanEqual => lhs.op_le(rhs, context),
        BinaryOperator::GreaterThan => lhs.op_gt(rhs, context),
        BinaryOperator::GreaterThanEqual => lhs.op_ge(rhs, context),
        BinaryOperator::BitwiseOr => lhs.op_bitor(rhs, context),
        BinaryOperator::BitwiseAnd => lhs.op_bitand(rhs, context),
        BinaryOperator::BitwiseXor => lhs.op_bitxor(rhs, context),
        BinaryOperator::ShiftLeft => lhs.op_shl(rhs, stage, context),
        BinaryOperator::ShiftRight => lhs.op_shr(rhs, stage, context),
    }
}

/// Call a unary operator.
///
/// The arguments must be [loaded][Type::loaded].
pub fn call_unary_op(operator: UnaryOperator, operand: &Instance) -> Result<Instance, E> {
    match operator {
        UnaryOperator::LogicalNegation => operand.op_not(),
        UnaryOperator::Negation => operand.op_neg(),
        UnaryOperator::BitwiseComplement => operand.op_bitnot(),
        UnaryOperator::AddressOf => operand.op_ref(),
        UnaryOperator::Indirection => operand.op_deref(),
    }
}

/// Constructs a builtin type.
///
/// Includes all builtin types such as `vec3<f32>`,
/// but does not include their predeclared aliases (`vec3f`).
pub fn builtin_type(
    name: &str,
    tplt: Option<&[TpltParam]>,
    context: &TyContext,
) -> Result<Type, E> {
    if let Some(t) = tplt {
        match name {
            "array" => Ok(ArrayTemplate::parse(t)?.ty()),
            #[cfg(feature = "naga-ext")]
            "binding_array" => Ok(crate::tplt::BindingArrayTemplate::parse(t)?.ty()),
            "vec2" => Ok(VecTemplate::parse(t, context)?.ty(2)),
            "vec3" => Ok(VecTemplate::parse(t, context)?.ty(3)),
            "vec4" => Ok(VecTemplate::parse(t, context)?.ty(4)),
            "mat2x2" => Ok(MatTemplate::parse(t)?.ty(2, 2)),
            "mat2x3" => Ok(MatTemplate::parse(t)?.ty(2, 3)),
            "mat2x4" => Ok(MatTemplate::parse(t)?.ty(2, 4)),
            "mat3x2" => Ok(MatTemplate::parse(t)?.ty(3, 2)),
            "mat3x3" => Ok(MatTemplate::parse(t)?.ty(3, 3)),
            "mat3x4" => Ok(MatTemplate::parse(t)?.ty(3, 4)),
            "mat4x2" => Ok(MatTemplate::parse(t)?.ty(4, 2)),
            "mat4x3" => Ok(MatTemplate::parse(t)?.ty(4, 3)),
            "mat4x4" => Ok(MatTemplate::parse(t)?.ty(4, 4)),
            "ptr" => Ok(PtrTemplate::parse(t, context)?.ty()),
            "atomic" => Ok(AtomicTemplate::parse(t)?.ty()),
            "texture_1d"
            | "texture_2d"
            | "texture_2d_array"
            | "texture_3d"
            | "texture_cube"
            | "texture_cube_array"
            | "texture_multisampled_2d"
            | "texture_storage_1d"
            | "texture_storage_2d"
            | "texture_storage_2d_array"
            | "texture_storage_3d" => Ok(Type::Texture(TextureTemplate::parse(name, t)?.ty())),

            #[cfg(feature = "naga-ext")]
            "texture_1d_array" | "texture_storage_1d_array" | "texture_multisampled_2d_array" => {
                Ok(Type::Texture(TextureTemplate::parse(name, t)?.ty()))
            }
            #[cfg(feature = "naga-ext")]
            "ray_query" => Ok(Type::RayQuery(None)),
            #[cfg(feature = "naga-ext")]
            // requires enable extensions
            "acceleration_structure" => Ok(Type::AccelerationStructure(Some(
                AccelerationStructureTags::parse(t)?,
            ))),

            _ => Err(E::UnexpectedTemplate(name.to_string())),
        }
    }
    // builtin types without a template
    else {
        match name {
            "bool" => Ok(Type::Bool),
            "__AbstractInt" => Ok(Type::AbstractInt),
            "__AbstractFloat" => Ok(Type::AbstractFloat),
            "i32" => Ok(Type::I32),
            "u32" => Ok(Type::U32),
            "f32" => Ok(Type::F32),
            "f16" => Ok(Type::F16),
            "texture_depth_multisampled_2d" => Ok(Type::Texture(TextureType::DepthMultisampled2D)),
            "texture_external" => Ok(Type::Texture(TextureType::External)),
            "texture_depth_2d" => Ok(Type::Texture(TextureType::Depth2D)),
            "texture_depth_2d_array" => Ok(Type::Texture(TextureType::Depth2DArray)),
            "texture_depth_cube" => Ok(Type::Texture(TextureType::DepthCube)),
            "texture_depth_cube_array" => Ok(Type::Texture(TextureType::DepthCubeArray)),
            "sampler" => Ok(Type::Sampler(SamplerType::Sampler)),
            "sampler_comparison" => Ok(Type::Sampler(SamplerType::SamplerComparison)),

            #[cfg(feature = "naga-ext")]
            "i64" => Ok(Type::I64),
            #[cfg(feature = "naga-ext")]
            "u64" => Ok(Type::U64),
            #[cfg(feature = "naga-ext")]
            "f64" => Ok(Type::F64),
            #[cfg(feature = "naga-ext")]
            "ray_query" => Ok(Type::RayQuery(Default::default())),
            #[cfg(feature = "naga-ext")]
            "acceleration_structure" => Ok(Type::AccelerationStructure(Default::default())),

            // Better error messages
            "array" => Err(E::MissingTemplate("array")),
            "binding_array" => Err(E::MissingTemplate("binding_array")),
            "vec2" => Err(E::MissingTemplate("vec2")),
            "vec3" => Err(E::MissingTemplate("vec3")),
            "vec4" => Err(E::MissingTemplate("vec4")),
            "mat2x2" => Err(E::MissingTemplate("mat2x2")),
            "mat2x3" => Err(E::MissingTemplate("mat2x3")),
            "mat2x4" => Err(E::MissingTemplate("mat2x4")),
            "mat3x2" => Err(E::MissingTemplate("mat3x2")),
            "mat3x3" => Err(E::MissingTemplate("mat3x3")),
            "mat3x4" => Err(E::MissingTemplate("mat3x4")),
            "mat4x2" => Err(E::MissingTemplate("mat4x2")),
            "mat4x3" => Err(E::MissingTemplate("mat4x3")),
            "mat4x4" => Err(E::MissingTemplate("mat4x4")),
            "ptr" => Err(E::MissingTemplate("ptr")),
            "atomic" => Err(E::MissingTemplate("atomic")),
            "texture_1d" => Err(E::MissingTemplate("texture_1d")),
            "texture_2d" => Err(E::MissingTemplate("texture_2d")),
            "texture_2d_array" => Err(E::MissingTemplate("texture_2d_array")),
            "texture_3d" => Err(E::MissingTemplate("texture_3d")),
            "texture_cube" => Err(E::MissingTemplate("texture_cube")),
            "texture_cube_array" => Err(E::MissingTemplate("texture_cube_array")),
            "texture_multisampled_2d" => Err(E::MissingTemplate("texture_multisampled_2d")),
            "texture_storage_1d" => Err(E::MissingTemplate("texture_storage_1d")),
            "texture_storage_2d" => Err(E::MissingTemplate("texture_storage_2d")),
            "texture_storage_2d_array" => Err(E::MissingTemplate("texture_storage_2d_array")),
            "texture_storage_3d" => Err(E::MissingTemplate("texture_storage_3d")),
            #[cfg(feature = "naga-ext")]
            "texture_1d_array" => Err(E::MissingTemplate("texture_1d_array")),
            #[cfg(feature = "naga-ext")]
            "texture_storage_1d_array" => Err(E::MissingTemplate("texture_storage_1d_array")),
            #[cfg(feature = "naga-ext")]
            "texture_multisampled_2d_array" => {
                Err(E::MissingTemplate("texture_multisampled_2d_array"))
            }

            _ => Err(E::UnknownType(name.to_string())),
        }
    }
}
