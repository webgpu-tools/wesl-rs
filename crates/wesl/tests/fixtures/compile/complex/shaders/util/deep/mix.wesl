// module `package::util::deep::mix`. Note that the intermediate module
// `package::util::deep` has no source file of its own: it is only a directory.

// chained super import: `super::super` = `package::util`, then reaches into
// its `ops` submodule.
import super::super::ops::double;

public fn mix2(x: f32) -> f32 {
    // inline import with a `super` prefix: `super::super` = `package::util`.
    return double(x) * 0.5 + super::super::dot2(vec2f(x), vec2f(1.0));
}
