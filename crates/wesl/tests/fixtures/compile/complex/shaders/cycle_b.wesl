// module `package::cycle_b`: the other half of the import cycle.

import package::cycle_a::A_OFFSET;

// included in the link because `pong` is used (const_asserts come along with
// the module whose items are used).
const_assert A_OFFSET > 0.0;

public fn pong(x: f32) -> f32 {
    return x * 2.0 + A_OFFSET;
}
