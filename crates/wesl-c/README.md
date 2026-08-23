# wesl-c

C bindings for the [`wesl-rs`][wesl-rs] compiler.

[Usage examples](./examples)

## Building

```bash
cargo build --package wesl-c --features eval,generics --release
# libraries should be located in target/release
```

[wesl-rs]: https://github.com/webgpu-tools/wesl-rs
