// Root module of the package (module path `package`).
// The test runner compiles this fixture twice:
//   run "toml_feat1": feat1 = true,  feat2 = false
//   run "toml_feat2": feat1 = false, feat2 = true
// and provides the virtual constants PI = 3.1415 and TRUE = true,
// plus the `random_wgsl` package (renamed `random` in wesl.toml).

// -- conditional import statements (@if/@else on imports) --------------------
// both bind the name `op`, only one survives conditional translation.
@if(feat1)
import package::util::ops::scale as op;
@else
import package::util::ops::shift as op;

// -- the `constants` virtual module ------------------------------------------
import constants::{PI, TRUE};

// -- import statements: single item, collections, nested collections,
// -- wildcard as a member of a collection ------------------------------------
import package::util::{dot2, ops::{double, *}}; // flattens to dot2, ops::double, ops::*

// wildcard import of a module that only contains re-exports (`public import`).
// brings `pdot2`, `x2` and `dbl` into scope.
import package::prelude::*;

// modules that circularly import each other (cycle_a <-> cycle_b).
import package::cycle_a::ping2;

// module testing shadowing rules.
import package::shadow::shadowed_fn;

// plain WGSL file used as a WESL module (.wgsl fallback).
import package::legacy::legacy_fn;

// deeply nested submodule (itself uses `super::super::` imports).
import package::util::deep::mix::mix2;

// package (external dependency) import statement.
import random::hash::rand11;

// helper from the module whose entry points are re-exported below.
import package::gfx::tonemap;

// -- visibility and re-exports (public imports) -------------------------------
// re-exported entry points, resource variable and override become part of the
// pipeline-visible API of this entry module, keeping their (aliased) names.
public import package::gfx::{vert_main, frag_main as fs_main};
public import package::gfx::scene_data;
public import package::gfx::exposure;

// -- conditional directives ---------------------------------------------------
@if(feat2)
diagnostic(off, derivative_uniformity);

// -- conditional global declarations ------------------------------------------
// @if/@elif/@else chain on global const declarations.
@if(feat1)
const K: f32 = 1.0;
@elif(feat2)
const K: f32 = 2.0;
@else
const K: f32 = 3.0;

// conditional type alias declarations. `Vec(0.5)` is valid with both.
@if(feat1)
alias Vec = vec2f;
@else
alias Vec = vec3f;

// conditional module-scope var, only referenced under the same condition.
@if(feat2)
var<private> counter: u32 = 0u;

// conditional override declarations.
@if(feat1)
override root_boost: f32 = 1.0;
@else
override root_boost: f32 = 2.0;

// visibility keywords on global declarations:
// `private` = visible only in this module, no keyword = package-visible.
private const ROOT_SECRET: f32 = 0.5;

// this *declaration* shares its name with the *module* `package::util`
// (util.wesl): modules and declarations live in separate namespaces.
// `gfx.wesl` imports `package::util` and uses both `util()` and `util::dot2`.
public fn util() -> f32 {
    return ROOT_SECRET * 8.0;
}

// -- conditional struct declarations and struct members ------------------------
@if(feat1)
struct Aux {
    n: u32,
}
@else
struct Aux {
    n: u32,
    extra_field: f32,
}

struct Config {
    size: f32,
    // @if/@elif/@else chain on struct members.
    @if(feat1)
    extra: vec2f,
    @elif(feat2)
    extra: vec3f,
    @else
    extra: f32,
}

// -- conditional global const assertions ---------------------------------------
const_assert TRUE; // uses the virtual bool constant
const_assert PI > 3.0;
@if(feat1)
const_assert K == 1.0;
@elif(feat2)
const_assert K == 2.0;

// -- conditional function declarations and formal parameters -------------------
// `boost` only exists when feat2 is enabled; call sites are guarded the same way.
fn weight(x: f32, @if(feat2) boost: f32) -> f32 {
    // conditional return statements.
    @if(feat2)
    return x * boost;
    @else
    return x;
}

// a second entry point, conditionally translated (workgroup size differs).
@if(feat1)
@compute @workgroup_size(1)
fn variant() {
    scene_data[0] = vec4f(K);
}
@else
@compute @workgroup_size(2)
fn variant() {
    scene_data[1] = vec4f(K);
}

@compute @workgroup_size(1)
fn my_compute(@builtin(global_invocation_id) gid: vec3u) {
    // -- inline imports (fully qualified paths in expressions) -----------------
    let inline_pkg = package::util::norm2(vec2f(1.0, 2.0));
    let inline_dep = random::noise::value::value_noise(2.0);

    // named imports and virtual constants.
    let a = dot2(vec2f(PI), vec2f(1.0));
    let b = double(a) + K + root_boost;
    let c = op(b); // conditionally-imported binding (scale or shift)
    let d = OPS_BASE; // brought in by the wildcard `ops::*`
    let e = pdot2(vec2f(d), vec2f(1.0)) + x2(vec2f(a), vec2f(b)) + dbl(c); // prelude re-exports
    let f = ping2(e); // circular modules
    let g = shadowed_fn(f); // shadowing tests
    let h = legacy_fn(g); // plain WGSL module
    let i = rand11(h) + mix2(inline_pkg) + tonemap(vec2f(inline_dep));

    // conditional local declarations (@if/@elif/@else on statements).
    @if(feat1)
    let cond = K * 1.0;
    @elif(feat2)
    let cond = K * 2.0;
    @else
    let cond = K * 3.0;

    var acc = 0.0;

    // conditional assignment / compound assignment statements.
    @if(feat1)
    acc += cond;
    @if(feat2)
    acc = cond;

    // conditional increment / phony-assignment on the feat2-only global.
    @if(feat2)
    counter++;
    @if(feat2)
    _ = counter;

    // conditional function call statement.
    @if(feat1)
    workgroupBarrier();

    // conditionally-translated struct construction, matching Config's members.
    @if(feat1)
    let cfg = Config(1.0, vec2f(0.0));
    @elif(feat2)
    let cfg = Config(1.0, vec3f(0.0));
    @else
    let cfg = Config(1.0, 0.0);
    acc += cfg.size;

    // conditional alias in use.
    let v = Vec(0.5);
    acc += v.x;

    @if(feat1)
    let aux = Aux(gid.x);
    @else
    let aux = Aux(gid.x, 0.5);
    acc += f32(aux.n);

    // guarded call of the function with a conditional parameter.
    @if(feat2)
    acc += weight(a, 2.0);
    @else
    acc += weight(a);

    // bool virtual constant in a regular expression.
    if TRUE {
        acc += 1.0;
    }

    // conditional switch clauses (@if/@elif on case clauses).
    switch gid.x {
        @if(feat1)
        case 1u: {
            acc += 1.0;
        }
        @elif(feat2)
        case 2u: {
            acc += 2.0;
        }
        @if(feat2)
        case 3u, 4u: {
            acc += 3.0;
        }
        default: {
            acc += 0.5;
        }
    }

    // conditional break / continue statements.
    for (var t = 0u; t < 4u; t++) {
        @if(feat2)
        continue;
        @if(feat1)
        break;
    }

    // conditional const assertion inside a function body.
    @if(feat1)
    const_assert 1 < 2;

    // write through the re-exported resource variable.
    scene_data[gid.x] = vec4f(acc + b + c + e + g + i);
}
