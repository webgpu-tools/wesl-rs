# wesl-rs-web

Runs the [`wesl-rs`][wesl-rs] compiler on the web.

It is used by [`wesl-playground`][wesl-playground], and provides most up-to-date
instructions on how to use this package.

## Usage

```ts
import InitWesl, * as Wesl from 'wesl-rs-web';

async function compile() {
  await InitWesl();

  const params: Wesl.Command = {
    command: 'Compile',
    entrypoint: 'package::main',
    files: {
      "package::main": "... source file of main ...",
    },
    // ... fill in other WESL compile options
  };

  try {
    const res = Wesl.run(params) as string // unfortunately Wesl.run returns `any`.
    console.log('compilation result', res)
  } catch (e) {
    console.error('compilation failure', e)
  }
}
```

## Building the package locally

You need [`wasm-pack`][wasm-pack] installed.

* release `wasm-pack build . --release --target web --out-dir path/to/dist/`
* development `wasm-pack build . --dev --target web --out-dir path/to/dist/ --features debug,naga`
That's for `wesl-playground`. you can switch the `--target` to `node` or `deno` or `bundler` depending on your
use-case. Read the [`wasm-pack` book][wasm-pack-book] for more.

[wesl-rs]: https://github.com/webgpu-tools/wesl-rs
[wesl-playground]: https://github.com/webgpu-tools/wesl-playground
[wasm-pack]: https://rustwasm.github.io/wasm-pack/
[wasm-pack-book]: https://rustwasm.github.io/docs/wasm-pack/
