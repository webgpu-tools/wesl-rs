// module `package::legacy`: a plain WGSL file consumed as a WESL module
// (used when no .wesl file exists for the module path).

fn legacy_fn(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}
