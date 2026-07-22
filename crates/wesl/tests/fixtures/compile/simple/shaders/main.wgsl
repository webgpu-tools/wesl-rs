// test fixture with only valid WGSL code. Contains most features of the language.

// directives
diagnostic(error, foo);
diagnostic(error, foo.bar, );
enable f16;
// enable clip_distances, subgroups, ;
requires unrestricted_pointer_parameters;
requires subgroup_id, buffer_view, ;

// global declarations
const c1 = 1;
const c2: u32 = 2;
override o1: f32;
override o2 = 0;
@id(10,) override o3: u32 = 0;
var<private> vp1 = 0;
var<private> vp2: u32 = 0;
var<workgroup> vw1: u32;
@group(0) @binding(1) var<uniform> vu1: u32;
@group(0) @binding(2) var vh1: texture_2d<f32>;
@group(0) @binding(3) var vh2: sampler;
@group(0) @binding(4) var<storage> vs1: u32;
@group(0) @binding(5) var<storage,read> vs2: u32;
@group(0) @binding(6) var<storage,read_write> vs4: u32;
@group(0) @binding(7) var<storage,read_write> vs5: atomic<u32>;
alias a1 = u32;
alias a2 = vec4<f32>;
alias a3 = array<f32, 4>;

// global const asserts
const_assert c1 == 1;
const_assert(c1 < c2);
;

// literals (used later as initializers)
const l1 = 0;
const l2 = 0i;
const l3 = 0u;
const l4 = 123;
const l5 = 456i;
const l6 = 789u;
const l7 = 0x0;
const l8 = 0xff;
const l9 = 0Xffu;
const l10 = 0xFFi;
const l11 = true;
const l12 = false;
const l13 = 0f;
const l14 = 0h;
const l15 = 1f;
const l16 = 1h;
const l17 = 0.5;
const l18 = .5;
const l19 = 1.;
const l20 = 1.5e10;
const l21 = 1.5e-10;
const l22 = 1.5E+10f;
const l23 = 0.5h;
const l24 = 1e10;
const l25 = 1e-10f;
const l26 = 0x1p4;
const l27 = 0x1.8p4;
const l28 = 0x.8p0;
const l29 = 0x1.p0f;
const l30 = 0x1p-4h;

// structs
struct S1 {
  m1: u32,
}

struct S2 {
  m1: u32,
  m2: f32,
}

struct S3 {
  @align(16) m1: u32,
  @size(32) m2: f32,
  @align(8) @size(4) m3: i32,
  m4: vec3<f32>,
  m5: array<u32, 4>,
  m6: S2,
}

struct S4 {
  @location(0) @interpolate(flat) m1: f32,
  @builtin(position) m2: vec4<f32>,
  @location(1) @interpolate(perspective, center) m3: f32,
  @location(2,) @interpolate(linear, centroid,) m4: f32,
  @location(3) @interpolate(flat, first) m5: f32,
  @invariant @builtin(position) m6: vec4<f32>,
}

// types (via alias declarations)
alias t1 = bool;
alias t2 = i32;
alias t3 = u32;
alias t4 = f32;
alias t5 = f16;
alias t6 = vec2<f32>;
alias t7 = vec3<i32>;
alias t8 = vec4<u32>;
alias t9 = vec2f;
alias t10 = vec3i;
alias t11 = vec4u;
alias t12 = vec2h;
alias t13 = mat2x2<f32>;
alias t14 = mat2x3<f32>;
alias t15 = mat2x4<f32>;
alias t16 = mat3x2<f32>;
alias t17 = mat3x3<f32>;
alias t18 = mat3x4<f32>;
alias t19 = mat4x2<f32>;
alias t20 = mat4x3<f32>;
alias t21 = mat4x4<f32>;
alias t22 = mat2x2f;
alias t23 = mat4x4h;
alias t24 = array<f32>;
alias t25 = array<f32, 8>;
alias t26 = array<array<f32, 2>, 3>;
alias t27 = atomic<u32>;
alias t28 = atomic<i32>;
alias t29 = ptr<function, u32>;
alias t30 = ptr<private, f32>;
alias t31 = ptr<workgroup, u32>;
alias t32 = ptr<storage, u32, read>;
alias t33 = ptr<storage, u32, read_write>;
alias t34 = ptr<uniform, vec4<f32>>;
alias t35 = texture_1d<f32>;
alias t36 = texture_2d<f32>;
alias t37 = texture_2d_array<f32>;
alias t38 = texture_3d<f32>;
alias t39 = texture_cube<f32>;
alias t40 = texture_cube_array<f32>;
alias t41 = texture_multisampled_2d<f32>;
alias t42 = texture_depth_2d;
alias t43 = texture_depth_2d_array;
alias t44 = texture_depth_cube;
alias t45 = texture_depth_cube_array;
alias t46 = texture_depth_multisampled_2d;
alias t47 = texture_storage_1d<rgba8unorm, write>;
alias t48 = texture_storage_2d<rgba8unorm, read>;
alias t49 = texture_storage_2d_array<rgba8unorm, read_write>;
alias t50 = texture_storage_3d<rgba32float, write>;
alias t51 = sampler;
alias t52 = sampler_comparison;
alias t53 = texture_external;

// function with no return type and empty body
fn f1() {}

// function with parameters
fn f2(p1: u32, p2: f32) {}

// function with attributes on params, and trailing comma
fn f3(@builtin(vertex_index) p1: u32, p2: f32,) -> u32 {
  return p1;
}

// function with return type and return attribute
fn f4() -> @location(0) vec4<f32> {
  return vec4<f32>(0.0);
}

// must_use attribute
@must_use
fn f5() -> u32 {
  return 0u;
}

// expressions
fn f7() {
  // literals
  let e1 = 1;
  let e2 = 1.0;
  let e3 = true;

  // identifier and template-elaborated identifier
  let e4 = c1;
  let e5 = vec2<f32>();

  // paren expression
  let e6 = (1 + 2);

  // call expressions and constructors
  let e7 = vec3<f32>(1.0, 2.0, 3.0);
  let e8 = vec3f(1.0);
  let e9 = mat2x2<f32>(1.0, 0.0, 0.0, 1.0);
  let e10 = array<u32, 3>(1u, 2u, 3u);
  let e11 = S2(1u, 2.0);
  let e12 = f3(0u, 0.0);
  let e13 = bitcast<u32>(1i);

  // unary operators
  let e14 = -1;
  let e15 = !true;
  let e16 = ~1u;
  var e17 = 1;
  let e18 = &e17;
  let e19 = *e18;

  // binary arithmetic
  let e20 = 1 + 2;
  let e21 = 1 - 2;
  let e22 = 1 * 2;
  let e23 = 1 / 2;
  let e24 = 1 % 2;

  // binary bitwise
  let e25 = 1u & 2u;
  let e26 = 1u | 2u;
  let e27 = 1u ^ 2u;
  let e28 = 1u << 2u;
  let e29 = 1u >> 2u;

  // comparison
  let e30 = 1 < 2;
  let e31 = 1 > 2;
  let e32 = 1 <= 2;
  let e33 = 1 >= 2;
  let e34 = 1 == 2;
  let e35 = 1 != 2;

  // short-circuit
  let e36 = true && false;
  let e37 = true || false;

  // precedence and nesting
  let e38 = 1 + 2 * 3 - 4 / 2;
  let e39 = (1 + 2) * (3 - 4);

  // component and swizzle access
  let e40 = vec4<f32>(1.0, 2.0, 3.0, 4.0);
  let e41 = e40.x;
  let e42 = e40.xy;
  let e43 = e40.xyz;
  let e44 = e40.xyzw;
  let e45 = e40.r;
  let e46 = e40.rgba;
  let e47 = e40[0];
  let e48 = e40.xy[1];

  // member access
  let e49 = e11.m1;
  let e50 = e11.m2;

  // array indexing chained
  var e51 = array<array<u32, 2>, 2>(array<u32, 2>(1u, 2u), array<u32, 2>(3u, 4u));
  let e52 = e51[0][1];
}

// statements
fn f8() {
  // empty statement
  ;

  // variable declarations (let, const, var)
  let s1 = 1;
  let s2: u32 = 1u;
  const s3 = 2;
  const s4: i32 = 2i;
  var s5 = 3;
  var s6: f32 = 3.0;
  var s7: u32;
  var<function> s8: u32;
  var<function> s9 = 0u;

  // assignments
  var s10 = 0;
  s10 = 1;
  s10 += 1;
  s10 -= 1;
  s10 *= 2;
  s10 /= 2;
  s10 %= 2;
  var s11 = 0u;
  s11 &= 1u;
  s11 |= 1u;
  s11 ^= 1u;
  s11 <<= 1u;
  s11 >>= 1u;

  // phony assignment
  _ = 1;
  _ = f5();

  // increment / decrement
  var s12 = 0;
  s12++;
  s12--;

  // assignment through pointer / lhs expressions
  var s13 = 0;
  let s14 = &s13;
  *s14 = 5;
  (*s14) = 6;

  // if statement
  if s10 > 0 {
    s10 = 0;
  }

  // if / else if / else
  if s10 == 0 {
    s10 = 1;
  } else if s10 == 1 {
    s10 = 2;
  } else {
    s10 = 3;
  }

  // if with attribute
  @diagnostic(off, derivative_uniformity)
  if true {
  }

  // switch statement
  switch s10 {
    case 0: {
      s10 = 1;
    }
    case 1, 2: {
      s10 = 2;
    }
    case 3, default: {
      s10 = 3;
    }
  }

  // switch with default alone and no colon
  switch s10 {
    default {
      s10 = 0;
    }
  }

  // loop statement with continuing and break if
  var s15 = 0;
  loop {
    s15 += 1;
    if s15 > 10 {
      break;
    }
    continuing {
      s15 += 1;
      break if s15 > 100;
    }
  }

  // loop with continue
  loop {
    if s15 > 0 {
      break;
    }
    continue;
  }

  // for statement
  for (var s16 = 0; s16 < 10; s16++) {
    s10 += s16;
  }

  // for with empty header parts
  for (;;) {
    break;
  }

  // for with variable_updating_statement
  for (s10++;;s10++) {
  }

  // for with func call init/update
  for (f1(); false; f1()) {
  }

  // while statement
  var s17 = 0;
  while s17 < 10 {
    s17 += 1;
  }

  // compound statement (nested block)
  {
    let s18 = 1;
    {
      let s19 = 2;
    }
  }

  // function call statement
  f1();
  f2(0u, 0.0);

  // const_assert inside function
  const_assert 1 < 2;
}

// function returning value with various return forms
fn f9(p1: bool) -> u32 {
  if p1 {
    return 1u;
  }
  return 0u;
}

// discard is only valid in fragment shaders
@fragment
fn f10() -> @location(0) vec4<f32> {
  if true {
    discard;
  }
  return vec4<f32>(0.0);
}

// entry points with stage attributes
@vertex
fn vert_main(@builtin(vertex_index) p1: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(f32(p1));
}

@fragment
fn frag_main(@builtin(position) p1: vec4<f32>, @location(0) @interpolate(flat) p2: f32) -> @location(0) vec4<f32> {
  return vec4<f32,>(p2);
}

@compute @workgroup_size(1)
fn comp_main1() {}

@compute @workgroup_size(8, 8)
fn comp_main2(@builtin(global_invocation_id) p1: vec3<u32>) {}

@compute @workgroup_size(8, 8, 1,)
fn comp_main3(
  @builtin(local_invocation_id) p1: vec3<u32>,
  @builtin(local_invocation_index) p2: u32,
  @builtin(workgroup_id) p3: vec3<u32>,
  @builtin(num_workgroups) p4: vec3<u32>,
) {}

// pointers as parameters
fn f11(p1: ptr<function, u32>) {
  *p1 = 1u;
}

fn f12() {
  var v1 = 0u;
  f11(&v1);
}

// nested member and swizzle write targets
fn f13() {
  var v1 = vec4<f32>(0.0);
  v1.x = 1.0;
  v1[0] = 2.0;
  v1.x += 1.0;

  var v2 = S3();
  v2.m1 = 1u;
  v2.m4.x = 1.0;
  v2.m5[0] = 1u;
}
