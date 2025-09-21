#![cfg(doc)]
#![allow(non_camel_case_types)]
// #![doc(html_no_source)]

use std::marker::PhantomData;

struct _Unknown;
/// Unknown WGSL type.
pub type Unknown = _Unknown;

struct _AbstractInt;
/// WGSL `AbstractInt` type.
pub type AbstractInt = _AbstractInt;

struct _AbstractFloat;
/// WGSL `AbstractFloat` type.
pub type AbstractFloat = _AbstractFloat;

struct _bool;
/// WGSL `bool` type.
pub type bool = _bool;

struct _i32;
/// WGSL `i32` type.
pub type i32 = _i32;

struct _u32;
/// WGSL `u32` type.
pub type u32 = _u32;

struct _f32;
/// WGSL `f32` type.
pub type f32 = _f32;

struct _f16;
/// WGSL `f16` type.
pub type f16 = _f16;

struct _vec2<T>(PhantomData<T>);
/// WGSL `vec2` type.
pub type vec2<T> = _vec2<T>;

struct _vec3<T>(PhantomData<T>);
/// WGSL `vec3` type.
pub type vec3<T> = _vec3<T>;

struct _vec4<T>(PhantomData<T>);
/// WGSL `vec4` type.
pub type vec4<T> = _vec4<T>;

struct _mat2x2<T>(PhantomData<T>);
/// WGSL `mat2x2` type.
pub type mat2x2<T> = _mat2x2<T>;

struct _mat2x3<T>(PhantomData<T>);
/// WGSL `mat2x3` type.
pub type mat2x3<T> = _mat2x3<T>;

struct _mat2x4<T>(PhantomData<T>);
/// WGSL `mat2x4` type.
pub type mat2x4<T> = _mat2x4<T>;

struct _mat3x2<T>(PhantomData<T>);
/// WGSL `mat3x2` type.
pub type mat3x2<T> = _mat3x2<T>;

struct _mat3x3<T>(PhantomData<T>);
/// WGSL `mat3x3` type.
pub type mat3x3<T> = _mat3x3<T>;

struct _mat3x4<T>(PhantomData<T>);
/// WGSL `mat3x4` type.
pub type mat3x4<T> = _mat3x4<T>;

struct _mat4x2<T>(PhantomData<T>);
/// WGSL `mat4x2` type.
pub type mat4x2<T> = _mat4x2<T>;

struct _mat4x3<T>(PhantomData<T>);
/// WGSL `mat4x3` type.
pub type mat4x3<T> = _mat4x3<T>;

struct _mat4x4<T>(PhantomData<T>);
/// WGSL `mat4x4` type.
pub type mat4x4<T> = _mat4x4<T>;

struct _atomic<T>(PhantomData<T>);
/// WGSL `atomic` type.
pub type atomic<T> = _atomic<T>;

struct _array<E, const N: usize>(PhantomData<E>);
/// WGSL `array` type.
pub type array<E, const N: usize = 0> = _array<E, N>;

struct _ptr<AS, T, AM>(PhantomData<AS>, PhantomData<T>, PhantomData<AM>);
/// WGSL `ptr` type.
pub type ptr<AS, T, AM> = _ptr<AS, T, AM>;

struct _texture_1d<T>(PhantomData<T>);
/// WGSL `texture_1d` type.
pub type texture_1d<T> = _texture_1d<T>;

struct _texture_2d<T>(PhantomData<T>);
/// WGSL `texture_2d` type.
pub type texture_2d<T> = _texture_2d<T>;

struct _texture_2d_array<T>(PhantomData<T>);
/// WGSL `texture_2d_array` type.
pub type texture_2d_array<T> = _texture_2d_array<T>;

struct _texture_3d<T>(PhantomData<T>);
/// WGSL `texture_3d` type.
pub type texture_3d<T> = _texture_3d<T>;

struct _texture_cube<T>(PhantomData<T>);
/// WGSL `texture_cube` type.
pub type texture_cube<T> = _texture_cube<T>;

struct _texture_cube_array<T>(PhantomData<T>);
/// WGSL `texture_cube_array` type.
pub type texture_cube_array<T> = _texture_cube_array<T>;

struct _texture_multisampled_2d<T>(PhantomData<T>);
/// WGSL `texture_multisampled_2d` type.
pub type texture_multisampled_2d<T> = _texture_multisampled_2d<T>;

struct _texture_depth_multisampled_2d;
/// WGSL `texture_depth_multisampled_2d` type.
pub type texture_depth_multisampled_2d = _texture_depth_multisampled_2d;

struct _texture_external;
/// WGSL `texture_external` type.
pub type texture_external = _texture_external;

struct _texture_storage_1d<Format, Access>(PhantomData<Format>, PhantomData<Access>);
/// WGSL `texture_storage_1d` type.
pub type texture_storage_1d<Format, Access> = _texture_storage_1d<Format, Access>;

struct _texture_storage_2d<Format, Access>(PhantomData<Format>, PhantomData<Access>);
/// WGSL `texture_storage_2d` type.
pub type texture_storage_2d<Format, Access> = _texture_storage_2d<Format, Access>;

struct _texture_storage_2d_storage_array<Format, Access>(PhantomData<Format>, PhantomData<Access>);
/// WGSL `texture_storage_2d_storage_array` type.
pub type texture_storage_2d_storage_array<Format, Access> =
    _texture_storage_2d_storage_array<Format, Access>;

struct _texture_storage_3d<Format, Access>(PhantomData<Format>, PhantomData<Access>);
/// WGSL `texture_storage_3d` type.
pub type texture_storage_3d<Format, Access> = _texture_storage_3d<Format, Access>;

struct _texture_depth_2d;
/// WGSL `texture_depth_2d` type.
pub type texture_depth_2d = _texture_depth_2d;

struct _texture_depth_2d_array;
/// WGSL `texture_depth_2d_array` type.
pub type texture_depth_2d_array = _texture_depth_2d_array;

struct _texture_depth_cube;
/// WGSL `texture_depth_cube` type.
pub type texture_depth_cube = _texture_depth_cube;

struct _texture_depth_cube_array;
/// WGSL `texture_depth_cube_array` type.
pub type texture_depth_cube_array = _texture_depth_cube_array;

struct _sampler;
/// WGSL `sampler` type.
pub type sampler = _sampler;

struct _sampler_comparison;
/// WGSL `sampler_comparison` type.
pub type sampler_comparison = _sampler_comparison;

// TYPE ALIASES

/// WGSL type alias of [`vec2<f32>`].
pub type vec2f = vec3<f32>;

/// WGSL type alias of [`vec3<f32>`].
pub type vec3f = vec3<f32>;

/// WGSL type alias of [`vec3<f32>`].
pub type vec4f = vec3<f32>;

/// WGSL type alias of [`vec3<i32>`].
pub type vec2i = vec3<i32>;

/// WGSL type alias of [`vec3<i32>`].
pub type vec3i = vec3<i32>;

/// WGSL type alias of [`vec3<i32>`].
pub type vec4i = vec3<i32>;

/// WGSL type alias of [`vec3<u32>`].
pub type vec2u = vec3<u32>;

/// WGSL type alias of [`vec3<u32>`].
pub type vec3u = vec3<u32>;

/// WGSL type alias of [`vec3<u32>`].
pub type vec4u = vec3<u32>;

/// WGSL type alias of [`mat2x2<f32>`].
pub type mat2x2f = mat2x2<f32>;

/// WGSL type alias of [`mat2x3<f32>`].
pub type mat2x3f = mat2x3<f32>;

/// WGSL type alias of [`mat2x4<f32>`].
pub type mat2x4f = mat2x4<f32>;

/// WGSL type alias of [`mat3x2<f32>`].
pub type mat3x2f = mat3x2<f32>;

/// WGSL type alias of [`mat3x3<f32>`].
pub type mat3x3f = mat3x3<f32>;

/// WGSL type alias of [`mat3x4<f32>`].
pub type mat3x4f = mat3x4<f32>;

/// WGSL type alias of [`mat4x2<f32>`].
pub type mat4x2f = mat4x2<f32>;

/// WGSL type alias of [`mat4x3<f32>`].
pub type mat4x3f = mat4x3<f32>;

/// WGSL type alias of [`mat4x4<f32>`].
pub type mat4x4f = mat4x4<f32>;

/// WGSL type alias of [`mat2x2<f16>`].
pub type mat2x2h = mat2x2<f16>;

/// WGSL type alias of [`mat2x3<f16>`].
pub type mat2x3h = mat2x3<f16>;

/// WGSL type alias of [`mat2x4<f16>`].
pub type mat2x4h = mat2x4<f16>;

/// WGSL type alias of [`mat3x2<f16>`].
pub type mat3x2h = mat3x2<f16>;

/// WGSL type alias of [`mat3x3<f16>`].
pub type mat3x3h = mat3x3<f16>;

/// WGSL type alias of [`mat3x4<f16>`].
pub type mat3x4h = mat3x4<f16>;

/// WGSL type alias of [`mat4x2<f16>`].
pub type mat4x2h = mat4x2<f16>;

/// WGSL type alias of [`mat4x3<f16>`].
pub type mat4x3h = mat4x3<f16>;

/// WGSL type alias of [`mat4x4<f16>`].
pub type mat4x4h = mat4x4<f16>;
