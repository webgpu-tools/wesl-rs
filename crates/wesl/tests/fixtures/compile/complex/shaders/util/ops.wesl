// module `package::util::ops`.

// super import: `super` = `package::util` (util.wesl), one level up.
import super::EPSILON;

// TODO: visibility
// public fn double(x: f32) -> f32 {
fn double(x: f32) -> f32 {
    return x * 2.0;
}

// package-visible items, bound in the root module via the conditional
// `import ... as op` statements.
fn scale(x: f32) -> f32 {
    return x * 4.0 + EPSILON;
}

fn shift(x: f32) -> f32 {
    return x + 4.0;
}

// picked up by the root module through the wildcard import `ops::*`.
const OPS_BASE: f32 = 4.0;

// const_asserts of this module are included because its items are used.
const_assert OPS_BASE > 0.0;
