<!-- markdownlint-disable MD041 -->

<!-- markdownlint-capture -->
<!-- markdownlint-disable -->
<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/wgsl-tooling-wg/wesl-spec/main/assets/logo/logo-square-dark.svg">
    <img width="200" height="200" src="https://raw.githubusercontent.com/wgsl-tooling-wg/wesl-spec/main/assets/logo/logo-square-light.svg" alt="WESL logo" />
  </picture>
</p>

<br/>

<p align="center">
  <a href="https://wesl-lang.dev">
    <img
      src="https://img.shields.io/badge/Documentation-0475b6?style=for-the-badge"
      alt="documentation web site"
      /></a>
  <a href="https://discord.gg/5UhkaSu4dt">
    <img
      src="https://img.shields.io/discord/1275293995152703488?style=for-the-badge&label=Discord"
      alt="wesl discord"
    /></a>
  <a href="https://crates.io/crates/wesl">
    <img
      src="https://img.shields.io/crates/v/wesl?style=for-the-badge"
      alt="crates.io wesl crate"
    /></a>
  <a href="https://docs.rs/wesl">
    <img
      src="https://img.shields.io/docsrs/wesl?style=for-the-badge"
      alt="docs.rs wesl crate"
    /></a>
</p>
<!-- markdownlint-restore -->

[docs-rs]: https://docs.rs/wesl
[discord]: https://discord.gg/5UhkaSu4dt
[tutorial]: https://wesl-lang.dev/docs/Getting-Started-Rust
[playground]: https://play.wesl-lang.dev

`wesl-rs` implements the necessary tools to build complex WGSL shaders, like what [naga_oil](https://github.com/bevyengine/naga_oil) does for [Bevy](https://bevyengine.org/), but in a framework-agnostic way. Visit [wesl-lang.dev](https://wesl-lang.dev/) to learn more about WGSL shader tools and language extensions.

## Usage

Read the [WESL for Rust tutorial][tutorial] and refer to the main crate [documentation][docs-rs].
Try out WESL and its implementations, `wesl-js` and `wesl-rs` on the [playground][playground].

## Status

> [!NOTE]
> last update: 2026-05

**WESL 0.2** was released and supports the following features:

* [x] Import statements & inline import paths
* [x] Conditional compilation with `@if`, `@elif`, `@else` attributes
* [x] Cargo shader packages

The following features are experimental:

* [x] Evaluation and Execution of WGSL code
* [x] Lowering of const-expressions, code normalization
* [x] Code validation

The following features are planned (to be designed with the WESL team):

* [ ] Automatic bindings, structs of bindings
* [ ] Namespaces / inline modules
* [ ] Generic functions

## Contributing

Contributions are welcome. Please join the [discord][discord] to get in touch with the community. Read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting Pull Requests.

## License

Except where noted (below and/or in individual files), all code in this repository is dual-licensed under either:

* MIT License ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.

### Your contributions

Unless you explicitly state otherwise,
any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license,
shall be dual licensed as above,
without any additional terms or conditions.
