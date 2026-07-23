// module `package::cycle_a`: circular module imports are allowed.
// cycle_a imports from cycle_b, and cycle_b imports from cycle_a.
// (the declarations themselves must not be cyclic: no call or const cycle.)

import package::cycle_b::pong;

const A_OFFSET: f32 = 0.25;

fn ping(x: f32) -> f32 {
    return x + A_OFFSET;
}

// TODO: visibility
// public fn ping2(x: f32) -> f32 {
fn ping2(x: f32) -> f32 {
    return pong(x) + ping(0.0);
}
