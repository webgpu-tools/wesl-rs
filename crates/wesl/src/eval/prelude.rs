use std::sync::LazyLock;

use wesl_quote::{quote_expression, quote_module};
use wgsl_parse::syntax::*;

use crate::idents::builtin_ident;

pub static EXPR_TRUE: Expression = quote_expression!(true);
pub static EXPR_FALSE: Expression = quote_expression!(false);
pub static ATTR_INTRINSIC: LazyLock<Attribute> = LazyLock::new(|| {
    Attribute::Custom(CustomAttribute {
        name: "__intrinsic".to_string(),
        arguments: None,
    })
});

pub static PRELUDE: LazyLock<TranslationUnit> = LazyLock::new(|| {
    let abstract_int = builtin_ident("__AbstractInt").unwrap();
    let abstract_float = builtin_ident("__AbstractFloat").unwrap();
    let mut module = quote_module! {
        // The prelude contains all pre-declared aliases, built-in structs and functions in WGSL.
        // the @__intrinsic attribute indicates that a function definition is defined by the compiler.
        // it means that it is not representable with user code: has generics, or variadics.

        alias vec2i = vec2<i32>;
        alias vec3i = vec3<i32>;
        alias vec4i = vec4<i32>;
        alias vec2u = vec2<u32>;
        alias vec3u = vec3<u32>;
        alias vec4u = vec4<u32>;
        alias vec2f = vec2<f32>;
        alias vec3f = vec3<f32>;
        alias vec4f = vec4<f32>;
        @extension(f16) alias vec2h = vec2<f16>;
        @extension(f16) alias vec3h = vec3<f16>;
        @extension(f16) alias vec4h = vec4<f16>;
        alias complexi = complex<i32>;
        alias complexu = complex<u32>;
        alias complexf = complex<f32>;
        @extension(f16) alias complexh = complex<f16>;
        alias quati = quat<i32>;
        alias quatu = quat<u32>;
        alias quatf = quat<f32>;
        @extension(f16) alias quath = quat<f16>;
        alias mat2x2f = mat2x2<f32>;
        alias mat2x3f = mat2x3<f32>;
        alias mat2x4f = mat2x4<f32>;
        alias mat3x2f = mat3x2<f32>;
        alias mat3x3f = mat3x3<f32>;
        alias mat3x4f = mat3x4<f32>;
        alias mat4x2f = mat4x2<f32>;
        alias mat4x3f = mat4x3<f32>;
        alias mat4x4f = mat4x4<f32>;
        @extension(f16) alias mat2x2h = mat2x2<f16>;
        @extension(f16) alias mat2x3h = mat2x3<f16>;
        @extension(f16) alias mat2x4h = mat2x4<f16>;
        @extension(f16) alias mat3x2h = mat3x2<f16>;
        @extension(f16) alias mat3x3h = mat3x3<f16>;
        @extension(f16) alias mat3x4h = mat3x4<f16>;
        @extension(f16) alias mat4x2h = mat4x2<f16>;
        @extension(f16) alias mat4x3h = mat4x3<f16>;
        @extension(f16) alias mat4x4h = mat4x4<f16>;

        // internal declarations are prefixed with __, which is not representable in WGSL source
        // therefore it avoids name collisions. AbstractInt and AbstractFloat too.
        struct __frexp_result_f32 { fract: f32, exp: i32 }
        struct __frexp_result_f16 { fract: f16, exp: i32 }
        struct __frexp_result_abstract { fract: #abstract_float, exp: #abstract_int }
        struct __frexp_result_vec2_f32 { fract: vec2<f32>, exp: vec2<i32> }
        struct __frexp_result_vec3_f32 { fract: vec3<f32>, exp: vec3<i32> }
        struct __frexp_result_vec4_f32 { fract: vec4<f32>, exp: vec4<i32> }
        @extension(f16) struct __frexp_result_vec2_f16 { fract: vec2<f16>, exp: vec2<i32> }
        @extension(f16) struct __frexp_result_vec3_f16 { fract: vec3<f16>, exp: vec3<i32> }
        @extension(f16) struct __frexp_result_vec4_f16 { fract: vec4<f16>, exp: vec4<i32> }
        struct __frexp_result_vec2_abstract { fract: vec2<#abstract_float>, exp: vec2<#abstract_int> }
        struct __frexp_result_vec3_abstract { fract: vec3<#abstract_float>, exp: vec3<#abstract_int> }
        struct __frexp_result_vec4_abstract { fract: vec4<#abstract_float>, exp: vec4<#abstract_int> }
        struct __modf_result_f32 { fract: f32, whole: f32 }
        struct __modf_result_f16 { fract: f16, whole: f16 }
        struct __modf_result_abstract { fract: #abstract_float, whole: #abstract_float }
        struct __modf_result_vec2_f32 { fract: vec2<f32>, whole: vec2<f32> }
        struct __modf_result_vec3_f32 { fract: vec3<f32>, whole: vec3<f32> }
        struct __modf_result_vec4_f32 { fract: vec4<f32>, whole: vec4<f32> }
        @extension(f16) struct __modf_result_vec2_f16 { fract: vec2<f16>, whole: vec2<f16> }
        @extension(f16) struct __modf_result_vec3_f16 { fract: vec3<f16>, whole: vec3<f16> }
        @extension(f16) struct __modf_result_vec4_f16 { fract: vec4<f16>, whole: vec4<f16> }
        struct __modf_result_vec2_abstract { fract: vec2<#abstract_float>, whole: vec2<#abstract_float> }
        struct __modf_result_vec3_abstract { fract: vec3<#abstract_float>, whole: vec3<#abstract_float> }
        struct __modf_result_vec4_abstract { fract: vec4<#abstract_float>, whole: vec4<#abstract_float> }
        @generic(T) struct __atomic_compare_exchange_result { old_value: T, exchanged: bool }

        // bitcast
        @const @must_use fn bitcast() @__intrinsic {}

        // logical
        @const @must_use fn all() @__intrinsic {}
        @const @must_use fn any() @__intrinsic {}
        @const @must_use fn select() @__intrinsic {}

        // array
        @const @must_use fn arrayLength() @__intrinsic {}

        // numeric
        @const @must_use fn abs() @__intrinsic {}
        @const @must_use fn acos() @__intrinsic {}
        @const @must_use fn acosh() @__intrinsic {}
        @const @must_use fn asin() @__intrinsic {}
        @const @must_use fn asinh() @__intrinsic {}
        @const @must_use fn atan() @__intrinsic {}
        @const @must_use fn atanh() @__intrinsic {}
        @const @must_use fn atan2() @__intrinsic {}
        @const @must_use fn ceil() @__intrinsic {}
        @const @must_use fn clamp() @__intrinsic {}
        @const @must_use fn cos() @__intrinsic {}
        @const @must_use fn cosh() @__intrinsic {}
        @const @must_use fn countLeadingZeros() @__intrinsic {}
        @const @must_use fn countOneBits() @__intrinsic {}
        @const @must_use fn countTrailingZeros() @__intrinsic {}
        @const @must_use fn cross() @__intrinsic {}
        @const @must_use fn degrees() @__intrinsic {}
        @const @must_use fn determinant() @__intrinsic {}
        @const @must_use fn distance() @__intrinsic {}
        @const @must_use fn dot() @__intrinsic {}
        @const @must_use fn dot4U8Packed() @__intrinsic {}
        @const @must_use fn dot4I8Packed() @__intrinsic {}
        @const @must_use fn exp() @__intrinsic {}
        @const @must_use fn exp2() @__intrinsic {}
        @const @must_use fn extractBits() @__intrinsic {}
        @const @must_use fn faceForward() @__intrinsic {}
        @const @must_use fn firstLeadingBit() @__intrinsic {}
        @const @must_use fn firstTrailingBit() @__intrinsic {}
        @const @must_use fn floor() @__intrinsic {}
        @const @must_use fn fma() @__intrinsic {}
        @const @must_use fn fract() @__intrinsic {}
        @const @must_use fn frexp() @__intrinsic {}
        @const @must_use fn insertBits() @__intrinsic {}
        @const @must_use fn inverseSqrt() @__intrinsic {}
        @const @must_use fn ldexp() @__intrinsic {}
        @const @must_use fn length() @__intrinsic {}
        @const @must_use fn log() @__intrinsic {}
        @const @must_use fn log2() @__intrinsic {}
        @const @must_use fn max() @__intrinsic {}
        @const @must_use fn min() @__intrinsic {}
        @const @must_use fn mix() @__intrinsic {}
        @const @must_use fn modf() @__intrinsic {}
        @const @must_use fn normalize() @__intrinsic {}
        @const @must_use fn pow() @__intrinsic {}
        @const @must_use fn quantizeToF16() @__intrinsic {}
        @const @must_use fn radians() @__intrinsic {}
        @const @must_use fn reflect() @__intrinsic {}
        @const @must_use fn refract() @__intrinsic {}
        @const @must_use fn reverseBits() @__intrinsic {}
        @const @must_use fn round() @__intrinsic {}
        @const @must_use fn saturate() @__intrinsic {}
        @const @must_use fn sign() @__intrinsic {}
        @const @must_use fn sin() @__intrinsic {}
        @const @must_use fn sinh() @__intrinsic {}
        @const @must_use fn smoothstep() @__intrinsic {}
        @const @must_use fn sqrt() @__intrinsic {}
        @const @must_use fn step() @__intrinsic {}
        @const @must_use fn tan() @__intrinsic {}
        @const @must_use fn tanh() @__intrinsic {}
        @const @must_use fn transpose() @__intrinsic {}
        @const @must_use fn trunc() @__intrinsic {}

        // derivative
        @must_use fn dpdx() @__intrinsic {}
        @must_use fn dpdxCoarse() @__intrinsic {}
        @must_use fn dpdxFine() @__intrinsic {}
        @must_use fn dpdy() @__intrinsic {}
        @must_use fn dpdyCoarse() @__intrinsic {}
        @must_use fn dpdyFine() @__intrinsic {}
        @must_use fn fwidth() @__intrinsic {}
        @must_use fn fwidthCoarse() @__intrinsic {}
        @must_use fn fwidthFine() @__intrinsic {}

        // texture
        @must_use fn textureDimensions() @__intrinsic {}
        @must_use fn textureGather() @__intrinsic {}
        @must_use fn textureGatherCompare() @__intrinsic {}
        @must_use fn textureLoad() @__intrinsic {}
        @must_use fn textureNumLayers() @__intrinsic {}
        @must_use fn textureNumLevels() @__intrinsic {}
        @must_use fn textureNumSamples() @__intrinsic {}
        @must_use fn textureSample() @__intrinsic {}
        @must_use fn textureSampleBias() @__intrinsic {}
        @must_use fn textureSampleCompare() @__intrinsic {}
        @must_use fn textureSampleCompareLevel() @__intrinsic {}
        @must_use fn textureSampleGrad() @__intrinsic {}
        @must_use fn textureSampleLevel() @__intrinsic {}
        @must_use fn textureSampleBaseClampToEdge() @__intrinsic {}
        fn textureStore() @__intrinsic {}

        // atomic
        fn atomicLoad() @__intrinsic {}
        fn atomicStore() @__intrinsic {}
        fn atomicAdd() @__intrinsic {}
        fn atomicSub() @__intrinsic {}
        fn atomicMax() @__intrinsic {}
        fn atomicMin() @__intrinsic {}
        fn atomicAnd() @__intrinsic {}
        fn atomicOr() @__intrinsic {}
        fn atomicXor() @__intrinsic {}
        fn atomicExchange() @__intrinsic {}
        fn atomicCompareExchangeWeak() @__intrinsic {}

        // packing
        @const @must_use fn pack4x8snorm() @__intrinsic { }
        @const @must_use fn pack4x8unorm() @__intrinsic {}
        @const @must_use fn pack4xI8() @__intrinsic {}
        @const @must_use fn pack4xU8() @__intrinsic {}
        @const @must_use fn pack4xI8Clamp() @__intrinsic {}
        @const @must_use fn pack4xU8Clamp() @__intrinsic {}
        @const @must_use fn pack2x16snorm() @__intrinsic {}
        @const @must_use fn pack2x16unorm() @__intrinsic {}
        @const @must_use fn pack2x16float() @__intrinsic {}
        @const @must_use fn unpack4x8snorm() @__intrinsic {}
        @const @must_use fn unpack4x8unorm() @__intrinsic {}
        @const @must_use fn unpack4xI8() @__intrinsic {}
        @const @must_use fn unpack4xU8() @__intrinsic {}
        @const @must_use fn unpack2x16snorm() @__intrinsic {}
        @const @must_use fn unpack2x16unorm() @__intrinsic {}
        @const @must_use fn unpack2x16float() @__intrinsic {}

        // synchronization
        fn storageBarrier() @__intrinsic {}
        fn textureBarrier() @__intrinsic {}
        fn workgroupBarrier() @__intrinsic {}
        @must_use fn workgroupUniformLoad() @__intrinsic {}

        // subgroup
        @extension(subgroups) @must_use fn subgroupAdd() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupExclusiveAdd() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupInclusiveAdd() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupAll() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupAnd() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupAny() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupBallot() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupBroadcast() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupBroadcastFirst() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupElect() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupMax() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupMin() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupMul() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupExclusiveMul() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupInclusiveMul() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupOr() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupShuffle() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupShuffleDown() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupShuffleUp() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupShuffleXor() @__intrinsic {}
        @extension(subgroups) @must_use fn subgroupXor() @__intrinsic {}

        // quad
        @extension(subgroups) @must_use fn quadBroadcast() @__intrinsic {}
        @extension(subgroups) @must_use fn quadSwapDiagonal() @__intrinsic {}
        @extension(subgroups) @must_use fn quadSwapX() @__intrinsic {}
        @extension(subgroups) @must_use fn quadSwapY() @__intrinsic {}

        // Naga ray tracing extension
        // https://github.com/gfx-rs/wgpu/blob/trunk/docs/api-specs/ray_tracing.md

        @extension(naga) struct RayDesc {
            flags: u32,
            cull_mask: u32,
            t_min: f32,
            t_max: f32,
            origin: vec3<f32>,
            dir: vec3<f32>,
        }

        @extension(naga) struct RayIntersection {
            kind: u32,
            t: f32,
            instance_custom_data: u32,
            instance_index: u32,
            sbt_record_offset: u32,
            geometry_index: u32,
            primitive_index: u32,
            barycentrics: vec2<f32>,
            front_face: bool,
            object_to_world: mat4x3<f32>,
            world_to_object: mat4x3<f32>,
        }

        // these are defined in naga/src/ir/mod.rs, structs RayFlag and RayQueryIntersection.
        @extension(naga) const RAY_FLAG_NONE = 0x0;
        @extension(naga) const RAY_FLAG_FORCE_OPAQUE = 0x1;
        @extension(naga) const RAY_FLAG_FORCE_NO_OPAQUE = 0x2;
        @extension(naga) const RAY_FLAG_TERMINATE_ON_FIRST_HIT = 0x4;
        @extension(naga) const RAY_FLAG_SKIP_CLOSEST_HIT_SHADER = 0x8;
        @extension(naga) const RAY_FLAG_CULL_BACK_FACING = 0x10;
        @extension(naga) const RAY_FLAG_CULL_FRONT_FACING = 0x20;
        @extension(naga) const RAY_FLAG_CULL_OPAQUE = 0x40;
        @extension(naga) const RAY_FLAG_CULL_NO_OPAQUE = 0x80;
        @extension(naga) const RAY_FLAG_SKIP_TRIANGLES = 0x100;
        @extension(naga) const RAY_FLAG_SKIP_AABBS = 0x200;
        @extension(naga) const RAY_QUERY_INTERSECTION_NONE = 0;
        @extension(naga) const RAY_QUERY_INTERSECTION_TRIANGLE = 1;
        @extension(naga) const RAY_QUERY_INTERSECTION_GENERATED = 2;
        @extension(naga) const RAY_QUERY_INTERSECTION_AABB = 3;


        // these are defined in naga/src/front/wgsl/lower/mod.rs, function call.
        @extension(naga) fn rayQueryInitialize() @__intrinsic {}
        @extension(naga) fn rayQueryProceed() -> bool @__intrinsic {}
        @extension(naga) fn rayQueryGenerateIntersection() @__intrinsic {}
        @extension(naga) fn rayQueryConfirmIntersection() @__intrinsic {}
        @extension(naga) fn rayQueryTerminate() @__intrinsic {}
        @extension(naga) fn rayQueryGetCommittedIntersection() -> RayIntersection @__intrinsic {}
        @extension(naga) fn rayQueryGetCandidateIntersection() -> RayIntersection @__intrinsic {}
        @extension(naga) fn getCommittedHitVertexPositions() -> array<vec3<f32>, 3> @__intrinsic {}
        @extension(naga) fn getCandidateHitVertexPositions() -> array<vec3<f32>, 3> @__intrinsic {}
    };
    crate::SyntaxUtil::retarget_idents(&mut module);
    module
});
