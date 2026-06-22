//! Static strings of all WGSL predeclared identifiers.

/// Built-in types identifiers.
/// Does not contain type-generators, see [`BUILTIN_TYPE_GENERATOR_NAMES`].
///
/// Reference: <https://www.w3.org/TR/WGSL/#predeclared-types>
pub const BUILTIN_TYPE_NAMES: &[&str] = &[
    // abstract types cannot be spelled in user code.
    // they are prefixed with `__`, which is not a valid WGSL identifier prefix.
    "__AbstractInt",
    "__AbstractFloat",
    // plain types
    "bool",
    "f16",
    "f32",
    "i32",
    "sampler",
    "sampler_comparison",
    "texture_depth_2d",
    "texture_depth_2d_array",
    "texture_depth_cube",
    "texture_depth_cube_array",
    "texture_depth_multisampled_2d",
    "texture_external",
    "u32",
    // naga extensions
    #[cfg(feature = "naga-ext")]
    "i64",
    #[cfg(feature = "naga-ext")]
    "u64",
    #[cfg(feature = "naga-ext")]
    "f64",
    #[cfg(feature = "naga-ext")]
    "ray_query",
    #[cfg(feature = "naga-ext")]
    "acceleration_structure",
];

/// Built-in type-generators identifiers.
///
/// Reference: <https://www.w3.org/TR/WGSL/#predeclared-types>
pub const BUILTIN_TYPE_GENERATOR_NAMES: &[&str] = &[
    "array",
    "atomic",
    "mat2x2",
    "mat2x3",
    "mat2x4",
    "mat3x2",
    "mat3x3",
    "mat3x4",
    "mat4x2",
    "mat4x3",
    "mat4x4",
    "ptr",
    "texture_1d",
    "texture_2d",
    "texture_2d_array",
    "texture_3d",
    "texture_cube",
    "texture_cube_array",
    "texture_multisampled_2d",
    "texture_storage_1d",
    "texture_storage_2d",
    "texture_storage_2d_array",
    "texture_storage_3d",
    "vec2",
    "vec3",
    "vec4",
    #[cfg(feature = "complex")]
    "complex",
    #[cfg(feature = "complex")]
    "quat",
    #[cfg(feature = "naga-ext")]
    "binding_array",
    #[cfg(feature = "naga-ext")]
    "texture_1d_array",
    #[cfg(feature = "naga-ext")]
    "texture_storage_1d_array",
    #[cfg(feature = "naga-ext")]
    "texture_multisampled_2d_array",
];

/// Built-in `struct` identifiers.
pub const BUILTIN_STRUCT_NAMES: &[&str] = &[
    // function return types cannot be spelled in user code.
    // they are prefixed with `__`, which is not a valid WGSL identifier prefix.
    "__frexp_result_f32",
    "__frexp_result_f16",
    "__frexp_result_abstract",
    "__frexp_result_vec2_f32",
    "__frexp_result_vec3_f32",
    "__frexp_result_vec4_f32",
    "__frexp_result_vec2_f16",
    "__frexp_result_vec3_f16",
    "__frexp_result_vec4_f16",
    "__frexp_result_vec2_abstract",
    "__frexp_result_vec3_abstract",
    "__frexp_result_vec4_abstract",
    "__modf_result_f32",
    "__modf_result_f16",
    "__modf_result_abstract",
    "__modf_result_vec2_f32",
    "__modf_result_vec3_f32",
    "__modf_result_vec4_f32",
    "__modf_result_vec2_f16",
    "__modf_result_vec3_f16",
    "__modf_result_vec4_f16",
    "__modf_result_vec2_abstract",
    "__modf_result_vec3_abstract",
    "__modf_result_vec4_abstract",
    "__atomic_compare_exchange_result",
    #[cfg(feature = "naga-ext")]
    "RayDesc",
    #[cfg(feature = "naga-ext")]
    "RayIntersection",
];

/// Built-in variable and value declarations identifiers.
///
/// There are none currently in WGSL.
pub const BUILTIN_DECLARATION_NAMES: &[&str] = &[
    // ray queries (naga extension). These are const declarations.
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_NONE", // value: 0x0
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_FORCE_OPAQUE", // value: 0x1
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_FORCE_NO_OPAQUE", // value: 0x2
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_TERMINATE_ON_FIRST_HIT", // value: 0x4
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_SKIP_CLOSEST_HIT_SHADER", // value: 0x8
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_CULL_BACK_FACING", // value: 0x10
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_CULL_FRONT_FACING", // value: 0x20
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_CULL_OPAQUE", // value: 0x40
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_CULL_NO_OPAQUE", // value: 0x80
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_SKIP_TRIANGLES", // value: 0x100
    #[cfg(feature = "naga-ext")]
    "RAY_FLAG_SKIP_AABBS", // value: 0x200
    #[cfg(feature = "naga-ext")]
    "RAY_QUERY_INTERSECTION_NONE", // value: 0
    #[cfg(feature = "naga-ext")]
    "RAY_QUERY_INTERSECTION_TRIANGLE", // value: 1
    #[cfg(feature = "naga-ext")]
    "RAY_QUERY_INTERSECTION_GENERATED", // value: 2
    #[cfg(feature = "naga-ext")]
    "RAY_QUERY_INTERSECTION_AABB", // value: 3
];

/// Predeclared type aliases names.
pub const BUILTIN_ALIAS_NAMES: &[&str] = &[
    "vec2i",
    "vec3i",
    "vec4i",
    "vec2u",
    "vec3u",
    "vec4u",
    "vec2f",
    "vec3f",
    "vec4f",
    "vec2h",
    "vec3h",
    "vec4h",
    #[cfg(feature = "complex")]
    "complexi",
    #[cfg(feature = "complex")]
    "complexu",
    #[cfg(feature = "complex")]
    "complexf",
    #[cfg(feature = "complex")]
    "complexh",
    #[cfg(feature = "complex")]
    "quati",
    #[cfg(feature = "complex")]
    "quatu",
    #[cfg(feature = "complex")]
    "quatf",
    #[cfg(feature = "complex")]
    "quath",
    "mat2x2f",
    "mat2x3f",
    "mat2x4f",
    "mat3x2f",
    "mat3x3f",
    "mat3x4f",
    "mat4x2f",
    "mat4x3f",
    "mat4x4f",
    "mat2x2h",
    "mat2x3h",
    "mat2x4h",
    "mat3x2h",
    "mat3x3h",
    "mat3x4h",
    "mat4x2h",
    "mat4x3h",
    "mat4x4h",
];

/// Built-in enumerants identifiers.
///
/// These enumerants are not context-dependent names, and can therefore be shadowed.
///
/// Reference: <https://www.w3.org/TR/WGSL/#predeclared-enumerants>
pub const BUILTIN_ENUMERANT_NAMES: &[&str] = &[
    // : access mode
    "read",
    "write",
    "read_write",
    // : address space (enumerant)
    "function",
    "private",
    "workgroup",
    "uniform",
    "storage",
    #[cfg(feature = "naga-ext")]
    "immediate",
    // : texel format
    "rgba8unorm",
    "rgba8snorm",
    "rgba8uint",
    "rgba8sint",
    "rgba16uint",
    "rgba16sint",
    "rgba16float",
    "r32uint",
    "r32sint",
    "r32float",
    "rg32uint",
    "rg32sint",
    "rg32float",
    "rgba32uint",
    "rgba32sint",
    "rgba32float",
    "bgra8unorm",
    #[cfg(feature = "naga-ext")]
    "r8unorm",
    #[cfg(feature = "naga-ext")]
    "r8snorm",
    #[cfg(feature = "naga-ext")]
    "r8uint",
    #[cfg(feature = "naga-ext")]
    "r8sint",
    #[cfg(feature = "naga-ext")]
    "r16unorm",
    #[cfg(feature = "naga-ext")]
    "r16snorm",
    #[cfg(feature = "naga-ext")]
    "r16uint",
    #[cfg(feature = "naga-ext")]
    "r16sint",
    #[cfg(feature = "naga-ext")]
    "r16float",
    #[cfg(feature = "naga-ext")]
    "rg8unorm",
    #[cfg(feature = "naga-ext")]
    "rg8snorm",
    #[cfg(feature = "naga-ext")]
    "rg8uint",
    #[cfg(feature = "naga-ext")]
    "rg8sint",
    #[cfg(feature = "naga-ext")]
    "rg16unorm",
    #[cfg(feature = "naga-ext")]
    "rg16snorm",
    #[cfg(feature = "naga-ext")]
    "rg16uint",
    #[cfg(feature = "naga-ext")]
    "rg16sint",
    #[cfg(feature = "naga-ext")]
    "rg16float",
    #[cfg(feature = "naga-ext")]
    "rgb10a2uint",
    #[cfg(feature = "naga-ext")]
    "rgb10a2unorm",
    #[cfg(feature = "naga-ext")]
    "rg11b10float",
    #[cfg(feature = "naga-ext")]
    "r64uint",
    #[cfg(feature = "naga-ext")]
    "rgba16unorm",
    #[cfg(feature = "naga-ext")]
    "rgba16snorm",
    #[cfg(feature = "naga-ext")]
    "vertex_return", // for ray_query and acceleration_structure
];

/// Built-in functions identifiers, excluding built-in constructors.
///
/// Reference: <https://www.w3.org/TR/WGSL/#builtin-functions>
pub const BUILTIN_FUNCTION_NAMES: &[&str] = &[
    // : bitcast
    "bitcast",
    // : logical
    "all",
    "any",
    "select",
    "arrayLength",
    "abs",
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atanh",
    "atan2",
    "ceil",
    "clamp",
    "cos",
    "cosh",
    "countLeadingZeros",
    "countOneBits",
    "countTrailingZeros",
    "cross",
    "degrees",
    "determinant",
    "distance",
    "dot",
    "dot4U8Packed",
    "exp",
    "exp2",
    "extractBits",
    "faceForward",
    "firstLeadingBit",
    "firstTrailingBit",
    "floor",
    "fma",
    "fract",
    "frexp",
    "insertBits",
    "inverseSqrt",
    "ldexp",
    "length",
    "log",
    "log2",
    "max",
    "min",
    "mix",
    "modf",
    "normalize",
    "pow",
    "quantizeToF16",
    "radians",
    "reflect",
    "refract",
    "reverseBits",
    "round",
    "saturate",
    "sign",
    "sin",
    "sinh",
    "smoothstep",
    "sqrt",
    "step",
    "tan",
    "tanh",
    "transpose",
    "trunc",
    // : derivative
    "dpdx",
    "dpdxCoarse",
    "dpdxFine",
    "dpdy",
    "dpdyCoarse",
    "dpdyFine",
    "fwidth",
    "fwidthCoarse",
    "fwidthFine",
    // : texture
    "textureDimensions",
    "textureGather",
    "textureGatherCompare",
    "textureLoad",
    "textureNumLayers",
    "textureNumLevels",
    "textureNumSamples",
    "textureSample",
    "textureSampleBias",
    "textureSampleCompare",
    "textureSampleCompareLevel",
    "textureSampleGrad",
    "textureSampleLevel",
    "textureSampleBaseClampToEdge",
    "textureStore",
    // : atomic
    "atomicLoad",
    "atomicStore",
    "atomicAdd",
    "atomicSub",
    "atomicMax",
    "atomicMin",
    "atomicAnd",
    "atomicOr",
    "atomicXor",
    "atomicExchange",
    "atomicCompareExchangeWeak",
    // : packing
    "pack4x8snorm",
    "pack4x8unorm",
    "pack4xI8",
    "pack4xU8",
    "pack4xI8Clamp",
    "pack4xU8Clamp",
    "pack2x16snorm",
    "pack2x16unorm",
    "pack2x16float",
    "unpack4x8snorm",
    "unpack4x8unorm",
    "unpack4xI8",
    "unpack4xU8",
    "unpack2x16snorm",
    "unpack2x16unorm",
    "unpack2x16float",
    // : synchronization
    "storageBarrier",
    "textureBarrier",
    "workgroupBarrier",
    "workgroupUniformLoad",
    // : subgroup
    "subgroupAdd",
    "subgroupExclusiveAdd",
    "subgroupInclusiveAdd",
    "subgroupAll",
    "subgroupAnd",
    "subgroupAny",
    "subgroupBallot",
    "subgroupBroadcast",
    "subgroupBroadcastFirst",
    "subgroupElect",
    "subgroupMax",
    "subgroupMin",
    "subgroupMul",
    "subgroupExclusiveMul",
    "subgroupInclusiveMul",
    "subgroupOr",
    "subgroupShuffle",
    "subgroupShuffleDown",
    "subgroupShuffleUp",
    "subgroupShuffleXor",
    "subgroupXor",
    // : quad
    "quadBroadcast",
    "quadSwapDiagonal",
    "quadSwapX",
    "quadSwapY",
    // : ray queries (naga extension)
    #[cfg(feature = "naga-ext")]
    "rayQueryInitialize",
    #[cfg(feature = "naga-ext")]
    "rayQueryProceed",
    #[cfg(feature = "naga-ext")]
    "rayQueryGenerateIntersection",
    #[cfg(feature = "naga-ext")]
    "rayQueryConfirmIntersection",
    #[cfg(feature = "naga-ext")]
    "rayQueryTerminate",
    #[cfg(feature = "naga-ext")]
    "rayQueryGetCommittedIntersection",
    #[cfg(feature = "naga-ext")]
    "rayQueryGetCandidateIntersection",
    #[cfg(feature = "naga-ext")]
    "getCommittedHitVertexPositions",
    #[cfg(feature = "naga-ext")]
    "getCandidateHitVertexPositions",
];

/// Built-in constructor identifiers (zero-value and value-constructors).
///
/// Reference: <https://www.w3.org/TR/WGSL/#constructor-builtin-function>
pub const BUILTIN_CONSTRUCTOR_NAMES: &[&str] = &[
    // constructor built-in functions
    "bool",
    "f16",
    "f32",
    "i32",
    "u32",
    #[cfg(feature = "naga-ext")]
    "i64",
    #[cfg(feature = "naga-ext")]
    "u64",
    #[cfg(feature = "naga-ext")]
    "f64",
    // type-generators
    "array",
    "mat2x2",
    "mat2x3",
    "mat2x4",
    "mat3x2",
    "mat3x3",
    "mat3x4",
    "mat4x2",
    "mat4x3",
    "mat4x4",
    "vec2",
    "vec3",
    "vec4",
    // predeclared aliases
    "vec2i",
    "vec3i",
    "vec4i",
    "vec2u",
    "vec3u",
    "vec4u",
    "vec2f",
    "vec3f",
    "vec4f",
    "vec2h",
    "vec3h",
    "vec4h",
    #[cfg(feature = "complex")]
    "complex",
    #[cfg(feature = "complex")]
    "complexi",
    #[cfg(feature = "complex")]
    "complexu",
    #[cfg(feature = "complex")]
    "complexf",
    #[cfg(feature = "complex")]
    "complexh",
    #[cfg(feature = "complex")]
    "quat",
    #[cfg(feature = "complex")]
    "quati",
    #[cfg(feature = "complex")]
    "quatu",
    #[cfg(feature = "complex")]
    "quatf",
    #[cfg(feature = "complex")]
    "quath",
    "mat2x2f",
    "mat2x3f",
    "mat2x4f",
    "mat3x2f",
    "mat3x3f",
    "mat3x4f",
    "mat4x2f",
    "mat4x3f",
    "mat4x4f",
    "mat2x2h",
    "mat2x3h",
    "mat2x4h",
    "mat3x2h",
    "mat3x3h",
    "mat3x4h",
    "mat4x2h",
    "mat4x3h",
    "mat4x4h",
    // ray queries (naga extension)
    #[cfg(feature = "naga-ext")]
    "RayDesc",
    #[cfg(feature = "naga-ext")]
    "RayIntersection",
];

/// All built-in identifiers.
pub fn iter_builtin_idents() -> impl Iterator<Item = &'static str> {
    itertools::chain!(
        BUILTIN_TYPE_NAMES.iter(),
        BUILTIN_TYPE_GENERATOR_NAMES.iter(),
        BUILTIN_STRUCT_NAMES.iter(),
        BUILTIN_DECLARATION_NAMES.iter(),
        BUILTIN_ALIAS_NAMES.iter(),
        BUILTIN_ENUMERANT_NAMES.iter(),
        BUILTIN_FUNCTION_NAMES.iter(),
        BUILTIN_CONSTRUCTOR_NAMES.iter(),
    )
    .map(|s| *s)
}

/// Reserved WGSL identifiers.
///
/// * WESL keywords (`as`, `import`, `super`, `self`) are included.
/// * `binding_array` is not included if feature `naga-ext` is set.
///
/// Reference: <https://www.w3.org/TR/WGSL/#reserved-words>
pub const RESERVED_WORDS: &[&str] = &[
    "NULL",
    "Self",
    "abstract",
    "active",
    "alignas",
    "alignof",
    "as",
    "asm",
    "asm_fragment",
    "async",
    "attribute",
    "auto",
    "await",
    "become",
    #[cfg(not(feature = "naga-ext"))]
    "binding_array",
    "cast",
    "catch",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "coherent",
    "column_major",
    "common",
    "compile",
    "compile_fragment",
    "concept",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "crate",
    "debugger",
    "decltype",
    "delete",
    "demote",
    "demote_to_helper",
    "do",
    "dynamic_cast",
    "enum",
    "explicit",
    "export",
    "extends",
    "extern",
    "external",
    "fallthrough",
    "filter",
    "final",
    "finally",
    "friend",
    "from",
    "fxgroup",
    "get",
    "goto",
    "groupshared",
    "highp",
    "impl",
    "implements",
    "import",
    "inline",
    "instanceof",
    "interface",
    "layout",
    "lowp",
    "macro",
    "macro_rules",
    "match",
    "mediump",
    "meta",
    "mod",
    "module",
    "move",
    "mut",
    "mutable",
    "namespace",
    "new",
    "nil",
    "noexcept",
    "noinline",
    "nointerpolation",
    "non_coherent",
    "noncoherent",
    "noperspective",
    "null",
    "nullptr",
    "of",
    "operator",
    "package",
    "packoffset",
    "partition",
    "pass",
    "patch",
    "pixelfragment",
    "precise",
    "precision",
    "premerge",
    "priv",
    "protected",
    "pub",
    "public",
    "readonly",
    "ref",
    "regardless",
    "register",
    "reinterpret_cast",
    "require",
    "resource",
    "restrict",
    "self",
    "set",
    "shared",
    "sizeof",
    "smooth",
    "snorm",
    "static",
    "static_assert",
    "static_cast",
    "std",
    "subroutine",
    "super",
    "target",
    "template",
    "this",
    "thread_local",
    "throw",
    "trait",
    "try",
    "type",
    "typedef",
    "typeid",
    "typename",
    "typeof",
    "union",
    "unless",
    "unorm",
    "unsafe",
    "unsized",
    "use",
    "using",
    "varying",
    "virtual",
    "volatile",
    "wgsl",
    "where",
    "with",
    "writeonly",
    "yield",
];
