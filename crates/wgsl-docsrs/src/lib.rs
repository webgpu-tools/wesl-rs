#![cfg(doc)]
#![allow(non_camel_case_types)]
#![doc(html_no_source)]

use std::marker::PhantomData;

/// Unknown WGSL type.
pub struct Unknown;

/// WGSL `AbstractInt` type.
pub struct AbstractInt;

/// WGSL `AbstractFloat` type.
pub struct AbstractFloat;

/// WGSL `bool` type.
pub struct bool;

/// WGSL `i32` type.
pub struct i32;

/// WGSL `u32` type.
pub struct u32;

/// WGSL `f32` type.
pub struct f32;

/// WGSL `f16` type.
pub struct f16;

/// WGSL `vec2` type.
pub struct vec2<T>(PhantomData<T>);

/// WGSL `vec3` type.
pub struct vec3<T>(PhantomData<T>);

/// WGSL `vec4` type.
pub struct vec4<T>(PhantomData<T>);

/// WGSL `mat2x2` type.
pub struct mat2x2<T>(PhantomData<T>);

/// WGSL `mat2x3` type.
pub struct mat2x3<T>(PhantomData<T>);

/// WGSL `mat2x4` type.
pub struct mat2x4<T>(PhantomData<T>);

/// WGSL `mat3x2` type.
pub struct mat3x2<T>(PhantomData<T>);

/// WGSL `mat3x3` type.
pub struct mat3x3<T>(PhantomData<T>);

/// WGSL `mat3x4` type.
pub struct mat3x4<T>(PhantomData<T>);

/// WGSL `mat4x2` type.
pub struct mat4x2<T>(PhantomData<T>);

/// WGSL `mat4x3` type.
pub struct mat4x3<T>(PhantomData<T>);

/// WGSL `mat4x4` type.
pub struct mat4x4<T>(PhantomData<T>);

/// WGSL `atomic` type.
pub struct atomic<T>(PhantomData<T>);

/// WGSL `array` type.
pub struct array<E, N>(PhantomData<E>, PhantomData<N>);

/// WGSL `ptr` type.
pub struct ptr<AS, T, AM>(PhantomData<AS>, PhantomData<T>, PhantomData<AM>);

/// WGSL `texture_1d` type.
pub struct texture_1d<T>(PhantomData<T>);

/// WGSL `texture_2d` type.
pub struct texture_2d<T>(PhantomData<T>);

/// WGSL `texture_2d_array` type.
pub struct texture_2d_array<T>(PhantomData<T>);

/// WGSL `texture_3d` type.
pub struct texture_3d<T>(PhantomData<T>);

/// WGSL `texture_cube` type.
pub struct texture_cube<T>(PhantomData<T>);

/// WGSL `texture_cube_array` type.
pub struct texture_cube_array<T>(PhantomData<T>);

/// WGSL `texture_multisampled_2d` type.
pub struct texture_multisampled_2d<T>(PhantomData<T>);

/// WGSL `texture_depth_multisampled_2d` type.
pub struct texture_depth_multisampled_2d;

/// WGSL `texture_external` type.
pub struct texture_external;

/// WGSL `texture_storage_1d` type.
pub struct texture_storage_1d<Format, Access>(PhantomData<Format>, PhantomData<Access>);

/// WGSL `texture_storage_2d` type.
pub struct texture_storage_2d<Format, Access>(PhantomData<Format>, PhantomData<Access>);

/// WGSL `texture_storage_2d_storage_array` type.
pub struct texture_storage_2d_storage_array<Format, Access>(
    PhantomData<Format>,
    PhantomData<Access>,
);

/// WGSL `texture_storage_3d` type.
pub struct texture_storage_3d<Format, Access>(PhantomData<Format>, PhantomData<Access>);

/// WGSL `texture_depth_2d` type.
pub struct texture_depth_2d;

/// WGSL `texture_depth_2d_array` type.
pub struct texture_depth_2d_array;

/// WGSL `texture_depth_cube` type.
pub struct texture_depth_cube;

/// WGSL `texture_depth_cube_array` type.
pub struct texture_depth_cube_array;

/// WGSL `sampler` type.
pub struct sampler;

/// WGSL `sampler_comparison` type.
pub struct sampler_comparison;

// TYPE ALIASES

/// WGSL `vec2f` type alias.
pub type vec2f = vec3<f32>;

/// WGSL `vec3f` type alias.
pub type vec3f = vec3<f32>;

/// WGSL `vec4f` type alias.
pub type vec4f = vec3<f32>;

/// WGSL `vec2i` type alias.
pub type vec2i = vec3<i32>;

/// WGSL `vec3i` type alias.
pub type vec3i = vec3<i32>;

/// WGSL `vec4i` type alias.
pub type vec4i = vec3<i32>;

/// WGSL `vec2u` type alias.
pub type vec2u = vec3<u32>;

/// WGSL `vec3u` type alias.
pub type vec3u = vec3<u32>;

/// WGSL `vec4u` type alias.
pub type vec4u = vec3<u32>;

/// WGSL `mat2x2f` type alias.
pub struct mat2x2f;

/// WGSL `mat2x3f` type alias.
pub struct mat2x3f;

/// WGSL `mat2x4f` type alias.
pub struct mat2x4f;

/// WGSL `mat3x2f` type alias.
pub struct mat3x2f;

/// WGSL `mat3x3f` type alias.
pub struct mat3x3f;

/// WGSL `mat3x4f` type alias.
pub struct mat3x4f;

/// WGSL `mat4x2f` type alias.
pub struct mat4x2f;

/// WGSL `mat4x3f` type alias.
pub struct mat4x3f;

/// WGSL `mat4x4f` type alias.
pub struct mat4x4f;

/// WGSL `mat2x2h` type alias.
pub struct mat2x2h;

/// WGSL `mat2x3h` type alias.
pub struct mat2x3h;

/// WGSL `mat2x4h` type alias.
pub struct mat2x4h;

/// WGSL `mat3x2h` type alias.
pub struct mat3x2h;

/// WGSL `mat3x3h` type alias.
pub struct mat3x3h;

/// WGSL `mat3x4h` type alias.
pub struct mat3x4h;

/// WGSL `mat4x2h` type alias.
pub struct mat4x2h;

/// WGSL `mat4x3h` type alias.
pub struct mat4x3h;

/// WGSL `mat4x4h` type alias.
pub struct mat4x4h;
