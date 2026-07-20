# WESL: A Community Standard for Enhanced WGSL

This is the crate for all your [WESL][wesl] needs.

See also the [standalone CLI][cli].

## Basic Usage

See [`Wesl`] for an overview of the high-level API.

```rust
# use wesl::{Wesl, VirtualResolver};
let compiler = Wesl::new("src/shaders");
# // just adding a virtual file here so the doctest runs without a filesystem
# let mut resolver = VirtualResolver::new();
# resolver.add_module("package::main".parse().unwrap(), "fn my_fn() {}".into());
# let compiler = compiler.set_custom_resolver(resolver);

// compile a WESL file to a WGSL string
let wgsl_str = compiler
    .compile(&"package::main".parse().unwrap())
    .inspect_err(|e| eprintln!("WESL error: {e}")) // pretty errors with `display()`
    .unwrap()
    .to_string();
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
# use wesl::Compiler;
fn main() {
    Compiler::new()
        .build_artifact("src/shaders/main.wgsl", "my_main_shader");
}
```

Include the compiled WGSL string in your code:

```rust,ignore
let module = device.create_shader_module(ShaderModuleDescriptor {
    label: Some("my_main_shader"),
    source: ShaderSource::Wgsl(include_wesl!("my_main_shader")),
});
```

## Write shaders inline with the [`quote_module`] macro

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
# use wesl::{Compiler, resolver::VirtualResolver, eval_str};
// ...standalone expression
let wgsl_expr = eval_str("abs(3 - 5)").unwrap().to_string();
assert_eq!(wgsl_expr, "2");

// ...expression using declarations in a WESL file
let source = "const my_const = 4; @const fn my_fn(v: u32) -> u32 { return v * 10; }";
# let mut resolver = VirtualResolver::new();
# resolver.add_module("package::source".parse().unwrap(), source.into());
# let compiler = Wesl::new_barebones().set_custom_resolver(resolver);
let wgsl_expr = compiler
    .compile(&"package::source".parse().unwrap()).unwrap()
    .eval("my_fn(my_const) + 2").unwrap()
    .to_string();
assert_eq!(wgsl_expr, "42u");
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
