// module `package::shadow`: local declarations shadowing global declarations
// and imported items.

import constants::PI;
import package::util::dot2;

const level: f32 = 1.0;

fn uses_globals() -> f32 {
    // module-scope names, not shadowed here.
    return level + PI + dot2(vec2f(1.0), vec2f(2.0));
}

public fn shadowed_fn(dot2: f32) -> f32 {
    // the formal parameter `dot2` shadows the imported function for the
    // entire body; the import is still usable through `uses_globals`.
    let base = uses_globals() + dot2;

    // local `let` shadows the global const `level`.
    let level = base + 1.0;

    // local `var` shadows the imported virtual constant `PI`.
    var PI = 3.0;

    {
        // nested-scope shadowing of the local above (initializer sees the
        // outer PI).
        let PI = PI * 2.0;
        return level + PI;
    }
}
