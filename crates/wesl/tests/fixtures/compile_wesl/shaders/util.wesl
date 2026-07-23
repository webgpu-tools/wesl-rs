// module `package::util` (which shares its name with `fn util` declared in the
// root module: modules and declarations live in separate namespaces).

// importing from one's own submodule requires a package-anchored path.
import package::util::ops::double;

// module attribute: marks this module as designed for wildcard imports.
// TODO: wildcards
// @!wildcardable;

// package-visible (default): usable from any module of this package,
// but not across package boundaries and not re-exportable.
const EPSILON: f32 = 1e-5;

// private: visible only within this module.
// TODO: visibility
// private const INTERNAL_SCALE: f32 = 2.0;
const INTERNAL_SCALE: f32 = 2.0;

// public: visible from any package, and re-exportable (see prelude.wesl).
// TODO: visibility
// public fn dot2(a: vec2f, b: vec2f) -> f32 {
fn dot2(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.x + a.y * b.y;
}

// TODO: visibility
// public fn cross2(a: vec2f, b: vec2f) -> f32 {
fn cross2(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.y - a.y * b.x;
}

// a public function may refer to less-visible items (EPSILON, INTERNAL_SCALE).
// TODO: visibility
// public fn norm2(a: vec2f) -> f32 {
fn norm2(a: vec2f) -> f32 {
    return sqrt(max(dot2(a, a), EPSILON)) * INTERNAL_SCALE / double(1.0);
}
