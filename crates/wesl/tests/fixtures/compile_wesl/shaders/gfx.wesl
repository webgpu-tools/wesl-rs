// module `package::gfx`: declares the pipeline-relevant items (entry points,
// resource variable, overrides) that the root module re-exports with
// `public import`.

// module import: `util` is used both as a module (`util::dot2`, the file
// util.wesl) and as a declaration (`util()`, the function declared in the
// root module package.wesl). See Imports.md "Import bindings".
import package::util;

// super import from a top-level module: `super` = `package` (the root module
// is `package.wesl`, so this is also a circular import: root imports gfx too).
import super::util::{cross2, EPSILON};

import constants::PI;

// a resource variable must be `public` to be re-exportable; the
// @group/@binding annotations carry through the re-export unchanged.
@group(0) @binding(0) public var<storage, read_write> scene_data: array<vec4f>;

// host-settable pipeline-overridable constant (re-exported by the root).
public override exposure: f32 = 1.0;

// a private override is not host-settable but may derive from another override.
private override gain: f32 = exposure * 2.0;

public fn tonemap(c: vec2f) -> f32 {
    return cross2(c, vec2f(EPSILON)) * gain;
}

// entry points must be `public` so the root module can re-export them.
@vertex
public fn vert_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    // `util` used as a declaration: `fn util` from the root module.
    return vec4f(f32(vi) * util(), 0.0, 0.0, 1.0);
}

@fragment
public fn frag_main(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    // conditional discard statement.
    @if(feat1)
    discard;
    // `util` used as a module: reaches into util.wesl.
    let m = util::dot2(pos.xy, pos.xy);
    return vec4f(m * PI + tonemap(pos.xy));
}
