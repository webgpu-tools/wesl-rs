# wesl-web

This is a `wasm-pack` shim to the [`wesl-rs`][wesl-rs] compiler.
It is used in [`wesl-playground`][wesl-playground].

It generates typescript bindings. Take inspiration from [`wesl-playground`][wesl-playground] for
how to use.

## Building

You need [`wasm-pack`][wasm-pack] installed.

* release `wasm-pack build . --release --target web --out-dir path/to/out/wesl-web`
* development `wasm-pack build . --dev --target web --out-dir path/to/out/wesl-web --features debug,naga`
That's for `wesl-playground`. you can switch the `--target` to `node` or `deno` depending on your
use-case. Read the [`wasm-pack` book][wasm-pack-book] for more.

[wesl-rs]: https://github.com/webgpu-tools/wesl-rs
[wesl-playground]: https://github.com/webgpu-tools/wesl-playground
[wasm-pack]: https://rustwasm.github.io/wasm-pack/
[wasm-pack-book]: https://rustwasm.github.io/docs/wasm-pack/
