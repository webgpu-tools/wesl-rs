# Command-Line Interface for `wesl-rs`

This is the frontend to the WESL compiler.

```text
Commands:
  check    Check correctness of the source file
  compile  Parse the source and convert it back to code from the syntax tree
  eval     Evaluate a const-expression
  exec     Execute a WGSL shader function on the CPU
  package  Generate a publishable Cargo package from WESL source code
  help     Print this message or the help of the given subcommand(s)
```

## Compiling a shader

The CLI by default outputs the compiled shader to stdout.
Pipe or redirect the output to a file if needed.

```bash
wesl compile <path/to/main.wesl> > compiled.wgsl
```

WESL reads from stdin if no input is provided.

```bash
cat <path/to/main.wesl> | wesl compile > compiled.wgsl
```

Flags allow customizing certain WESL features, see `wesl compile --help`.

## Evaluating a const-expression

The CLI allows evaluating const-expressions.

```bash
wesl eval "1 + abs(-1)"
# expected output: 2
```

Provide a context program for the expression. Enable the `--no-strip` switch
to prevent dead-code elimination. Functions must be `@const` to be executed in
constant-expressions.

```bash
echo "@const fn return_two() -> u32 { return 2; }" | wesl eval --no-strip "return_two()"
# expected output: 2u
```

## Shader execution on the CPU (experimental)

The `wesl exec` command allow evaluation of entrypoint functions using the
WESL compiler. It allows inspecting specific return values and catching runtime
errors early. This functionality is experimental, not all built-in functions
are implemented.

```bash
wesl exec <path/to/shader.wesl> --entrypoint fs_main
```

You can bind binary buffers to bindings (`--resource`) and set values of
pipeline-overridable constants (`--override`). You can also output the result as
a binary buffer following the WebGPU layout (`--out-binary`).

## Packaging shader libraries

The packaging command outputs to stdout the Rust codegen for the package. This
is the same output produced by [`wesl::PackageBuilder`] in `build.rs` files. Using the
CLI instead can be useful if you don't want to rely on a build.rs. Refer to the
[WESL] documentation about packaging for more information.

```bash
wesl package mypkgname <path/to/package/root/directory> > mypkgname.rs
```

[WESL]: https://docs.rs/crate/wesl
