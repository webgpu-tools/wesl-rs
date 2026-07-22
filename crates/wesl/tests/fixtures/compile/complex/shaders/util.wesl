// module `package::util` (which shares its name with `fn util` declared in the
// root module: modules and declarations live in separate namespaces).

// importing from one's own submodule requires a package-anchored path.
import package::util::ops::double;

// module attribute: marks this module as designed for wildcard imports.
@!wildcardable;

// package-visible (default): usable from any module of this package,
// but not across package boundaries and not re-exportable.
const EPSILON: f32 = 1e-5;

// private: visible only within this module.
private const INTERNAL_SCALE: f32 = 2.0;

// public: visible from any package, and re-exportable (see prelude.wesl).
public fn dot2(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.x + a.y * b.y;
}

public fn cross2(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.y - a.y * b.x;
}

// a public function may refer to less-visible items (EPSILON, INTERNAL_SCALE).
public fn norm2(a: vec2f) -> f32 {
    return sqrt(max(dot2(a, a), EPSILON)) * INTERNAL_SCALE / double(1.0);
}
