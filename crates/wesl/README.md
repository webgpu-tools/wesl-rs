# WESL: A Community Standard for Enhanced WGSL

This is the crate for all your [WESL][wesl] needs.

See also the [standalone CLI][cli].

## Basic Usage

See [`Compiler`] for an overview of the high-level API.

```rust
# use wesl::{Compiler, resolver::VirtualResolver};
#
let compiler = Compiler::default();
#
# // just adding a virtual file here so the doctest runs without a filesystem
# let mut resolver = VirtualResolver::new();
# let shader_string = "fn my_fn() {\n\n}\n";
# resolver.add_module("package::path::to::shader".parse().unwrap(), shader_string.into());
# let mut compiler = compiler.with_resolver(resolver);
# compiler.options.keep_main = true;
#
let compile_result = compiler
    .compile("path/to/shader.wesl")
    .inspect_err(|e| eprintln!("{e}")) // pretty-print errors
    .expect("compilation error");
let wgsl_string = compile_result.syntax.to_string();
#
# assert!(&wgsl_string == shader_string);
```

## Usage in [`build.rs`](https://doc.rust-lang.org/cargo/reference/build-scripts.html)

In your Rust project you probably want to have your WESL code converted automatically
to a WGSL string at build-time, unless your WGSL code must be assembled at runtime.

Add this crate to your build dependencies in `Cargo.toml`:

```toml
[build-dependencies]
wesl = "0.1"
```

Create the `build.rs` file with the following content:

```rust,no_run
# use wesl::{Compiler, resolver::VirtualResolver};
fn main() {
    let compiler = Compiler::default();
    let compile_result = compiler
        .compile("src/shaders/main.wesl")
        .inspect_err(|e| eprintln!("{e}")) // pretty-print errors
        .expect("compilation error");

    compile_result.emit_rerun_if_changed(); // optional, it prevents re-running the script if the shader have not changed
    compile_result.write_artifact("my_main_shader"); // writes the compiled file to `$OUT_DIR/my_main_shader.wesl`.
}
```

Include the compiled WGSL string in your code:

```rust,ignore
let module = device.create_shader_module(ShaderModuleDescriptor {
    label: Some("my_main_shader"),
    source: ShaderSource::Wgsl(include_wesl!("my_main_shader")), // `include_wesl` is a tiny convenience macro to load `$OUT_DIR/my_main_shader.wesl`.
});
```

## Write shaders inline with the `quote_module` macro

See the [`wesl-quote`][wesl-quote] crate.

## Evaluating const-expressions

This is an advanced and experimental feature. `wesl-rs` supports evaluation and execution
of WESL code with the `eval` feature flag. Early evaluation (in particular of
const-expressions) helps developers to catch bugs early by improving the validation and
error reporting capabilities of WESL. Full evaluation of const-expressions can be enabled
with the `lower` compiler option.

Additionally, the `eval` feature adds support for user-defined `@const` attributes on
functions, which allows one to precompute data ahead of time, and ensure that code has no
runtime dependencies.

The eval/exec implementation is tested with the [WebGPU Conformance Test Suite][cts].

```rust
# #[cfg(feature = "eval")] { // feature-gate
# use wesl::{Compiler, resolver::VirtualResolver, eval_str};
// ...standalone expression
let wgsl_expr = eval_str("abs(3 - 5)").unwrap().to_string();
assert_eq!(wgsl_expr, "2");

// ...expression using declarations in a WESL file
let source = "const my_const = 4; @const fn my_fn(v: u32) -> u32 { return v * 10; }";
#
# let mut resolver = VirtualResolver::new();
# resolver.add_module("package::main".parse().unwrap(), source.into());
# let mut compiler = Compiler::default().with_resolver(resolver);
# compiler.options.keep_main = true; // prevent dead code elimination
#
let wgsl_expr = compiler
    .compile("main.wgsl").unwrap()
    .eval("my_fn(my_const) + 2").unwrap()
    .to_string();
assert_eq!(wgsl_expr, "42u");
# } // end feature-gate
```

## Features

| name       | description                                           | Status/Specification      |
|------------|-------------------------------------------------------|---------------------------|
| `generics` | user-defined type-generators and generic functions    | [experimental][generics]  |
| `package`  | create shader libraries published to `crates.io`      | [experimental][packaging] |
| `eval`     | execute shader code on the CPU and `@const` attribute | experimental              |
| `naga-ext` | enable all Naga/WGPU extensions                       | experimental              |
| `serde`    | derive `Serialize` and `Deserialize` for syntax nodes |                           |

[wesl]: https://wesl-lang.dev
[wesl-quote]: https://docs.rs/wesl-quote
[cli]: https://crates.io/crates/wesl-cli
[generics]: https://github.com/k2d222/wesl-spec/blob/generics/Generics.md
[packaging]: https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/Packaging.md
[cts]: https://github.com/k2d222/wesl-cts
