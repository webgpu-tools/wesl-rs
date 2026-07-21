# Write shaders inline with the [`quote_module`] macro

This crate provides the `quote_*!` macros which let one write WGSL shaders
directly in source code. *This is experimental work*.

Writing shaders inline in Rust code has a few advantages:

* Like [`quote`](https://docs.rs/quote), it supports local variable injection. This can be
  used e.g. to customize a shader module at runtime.
* The module is parsed at build-time, and syntax errors will be reported at the right
  location in the macro invocation.
* WGSL and Rust have a similar syntax. Your Rust syntax highlighter will also highlight
  injected WGSL code.

```rust
# use wesl_quote::quote_module;
use wesl::syntax::*; // this is necessary for the quote_module macro

// this i64 value is computed at runtime and injected into the shader.
let num_iterations = 8i64;

// the following variable has type `TranslationUnit`.
let wgsl = quote_module! {
    @fragment
    fn fs_main(@location(0) in_color: vec4<f32>) -> @location(0) vec4<f32> {
        for (let i = 0; i < #num_iterations; i++) {
            // ...
        }
        return in_color;
    }
};
```

One can inject variables into the following places by prefixing the name with
a `#` symbol:

| Code location | Injected type | Indirectly supported injection type with `Into` |
| ------------- | ------------- | ----------------------------------------------- |
| name of a global declaration | `GlobalDeclaration` | `Declaration` `TypeAlias` `Struct` `Function` `ConstAssert` |
| name of a struct member | `StructMember` | |
| name of an attribute, after `@` | `Attribute` | `BuiltinValue` `InterpolateAttribute` `WorkgroupSizeAttribute` `TypeConstraint` `CustomAttribute` |
| type or identifier expression | `Expression` | `LiteralExpression` `ParenthesizedExpression` `NamedComponentExpression` `IndexingExpression` `UnaryExpression` `BinaryExpression` `FunctionCallExpression` `TypeOrIdentifierExpression` and transitively: `bool` `i64` (AbstractInt) `f64` (AbstractFloat) `i32` `u32` `f32` `Ident` |
| name of an attribute preceding and empty block statement | `Statement` | `CompoundStatement` `AssignmentStatement` `IncrementStatement` `DecrementStatement` `IfStatement` `SwitchStatement` `LoopStatement` `ForStatement` `WhileStatement` `BreakStatement` `ContinueStatement` `ReturnStatement` `DiscardStatement` `FunctionCallStatement` `ConstAssertStatement` `DeclarationStatement` |

```rust
# use wesl_quote::quote_module;
use wesl::syntax::*; // this is necessary for the quote_module macro

let inject_struct = Struct::new(Ident::new("mystruct".to_string()));
let inject_func = Function::new(Ident::new("myfunc".to_string()));
let inject_stmt = Statement::Void;
let inject_expr = 1f32;
let wgsl = quote_module! {
    struct #inject_struct { dummy: u32 } // structs cannot be empty
    fn #inject_func() {}
    fn foo() {
        @#inject_stmt {}
        let x: f32 = #inject_expr;
    }
};
```
