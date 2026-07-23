# wgsl-parse

<!-- markdownlint-disable reference-links-images -->

A parser and syntax tree for WGSL files, written directly from the [specification] with [lalrpop].

It supports WESL language extensions guarded by feature flags.

## WESL Features

| name         | description                                    | WESL Specification       |
|--------------|------------------------------------------------|--------------------------|
| `wesl`       | enable all WESL extensions below               |                          |
| `imports`    | `import` statements and inline qualified paths | [complete][imports]      |
| `attributes` | extra attributes locations on statements       | [complete][condcomp]     |
| `condcomp`   | `@if` attributes                               | [complete][condcomp]     |
| `generics`   | `@type` attributes                             | [experimental][generics] |

## Other Features

| name       | description                                                |
|------------|------------------------------------------------------------|
| `naga-ext` | enable all Naga/WGPU extensions (experimental)             |
| `serde`    | derive `Serialize` and `Deserialize` for syntax tree nodes |
| `tokrepr`  | derive [`TokRepr`][tokrepr] for syntax tree nodes          |

## Parsing and Stringification

[`TranslationUnit`][syntax::TranslationUnit] implements [`FromStr`][std::str::FromStr].
Other syntax nodes also implement `FromStr`: `GlobalDirective`, `GlobalDeclaration`, `Statement`, `Expression` and `ImportStatement`.

The syntax tree elements implement [`Display`][std::fmt::Display].
The display is always pretty-printed.

```rust
# use wgsl_parse::syntax::TranslationUnit;
# use std::str::FromStr;
let source = "@fragment fn frag_main() -> @location(0) vec4f { return vec4(1); }";
let mut module = TranslationUnit::from_str(source).unwrap();
// modify the module as needed...
println!("{module}");
```

[lalrpop]: https://lalrpop.github.io/lalrpop/
[specification]: https://www.w3.org/TR/WGSL/
[imports]: https://wesl-lang.dev/spec/Imports
[condcomp]: https://wesl-lang.dev/spec/ConditionalTranslation
[generics]: https://github.com/k2d222/wesl-spec/blob/generics/Generics.md
[tokrepr]: https://docs.rs/tokrepr
