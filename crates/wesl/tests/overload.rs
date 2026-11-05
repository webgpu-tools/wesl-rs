//! Integration tests for function overloads.

use wesl::{
    CompileOptions, Compiler, StrippingOptions, resolver::VirtualResolver, syntax::ModulePath,
};

fn compile(
    main: &str,
    modules: &[(&str, &str)],
    options: CompileOptions,
) -> Result<String, String> {
    let mut resolver = VirtualResolver::new();
    resolver.add_module(ModulePath::new_root(), main.to_string().into());
    for (path, source) in modules {
        resolver.add_module(path.parse().unwrap(), source.to_string().into());
    }
    let compiler = Compiler::new_with_resolver(options, resolver);
    compiler
        .compile_root()
        .map(|res| res.syntax.to_string())
        .map_err(|e| e.to_string())
}

fn compile_main(main: &str) -> Result<String, String> {
    let options = CompileOptions {
        strip: Some(StrippingOptions::KeepMainModule),
        ..Default::default()
    };
    compile(main, &[], options)
}

/// Remove whitespace to make comparisons insensitive to formatting.
fn normalize(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[track_caller]
fn assert_compiles_to(main: &str, expected: &str) {
    let res = compile_main(main)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    assert_eq!(normalize(&res), normalize(expected), "\n{res}");
}

#[track_caller]
fn assert_fails(main: &str, msg: &str) {
    let err = compile_main(main).expect_err("expected compilation failure");
    assert!(err.contains(msg), "error `{err}` does not contain `{msg}`");
}

#[test]
fn exact_match() {
    assert_compiles_to(
        "fn foo(x: f32) -> f32 { return x; }
         fn foo(x: i32) -> i32 { return x; }
         fn main() { let a = foo(1f); let b = foo(1i); }",
        "fn foo_f32(x: f32) -> f32 { return x; }
         fn foo_i32(x: i32) -> i32 { return x; }
         fn main() { let a = foo_f32(1f); let b = foo_i32(1i); }",
    );
}

#[test]
fn conversion_rank() {
    // AbstractInt -> i32 (rank 3) is preferred over AbstractInt -> f32 (rank 6).
    // AbstractFloat -> f32 is the only feasible conversion.
    assert_compiles_to(
        "fn foo(x: f32) {}
         fn foo(x: i32) {}
         fn main() { foo(1); foo(1.0); }",
        "fn foo_f32(x: f32) {}
         fn foo_i32(x: i32) {}
         fn main() { foo_i32(1); foo_f32(1.0); }",
    );
}

#[test]
fn multiple_params() {
    assert_compiles_to(
        "fn foo(x: f32, y: f32) {}
         fn foo(x: u32, y: f32) {}
         fn main() { let x = 1u; foo(x, 1); foo(1.0, 1); }",
        "fn foo_f32_f32(x: f32, y: f32) {}
         fn foo_u32_f32(x: u32, y: f32) {}
         fn main() { let x = 1u; foo_u32_f32(x, 1); foo_f32_f32(1.0, 1); }",
    );
}

#[test]
fn param_count() {
    assert_compiles_to(
        "fn foo() {}
         fn foo(x: f32) {}
         fn main() { foo(); foo(1.0); }",
        "fn foo() {}
         fn foo_f32(x: f32) {}
         fn main() { foo(); foo_f32(1.0); }",
    );
}

#[test]
fn different_return_types() {
    assert_compiles_to(
        "fn foo(x: f32) -> f32 { return x; }
         fn foo(x: vec2f) -> vec2f { return x; }
         fn main() { let a: vec2f = foo(vec2f()); }",
        "fn foo_f32(x: f32) -> f32 { return x; }
         fn foo_vec2_f32(x: vec2f) -> vec2f { return x; }
         fn main() { let a: vec2f = foo_vec2_f32(vec2f()); }",
    );
}

#[test]
fn nested_calls() {
    // the inner call return type selects the outer overload.
    assert_compiles_to(
        "fn foo(x: f32) -> i32 { return 1; }
         fn foo(x: i32) -> f32 { return 1.0; }
         fn main() { let a = foo(foo(1.0)); }",
        "fn foo_f32(x: f32) -> i32 { return 1; }
         fn foo_i32(x: i32) -> f32 { return 1.0; }
         fn main() { let a = foo_i32(foo_f32(1.0)); }",
    );
}

#[test]
fn overload_calls_overload() {
    assert_compiles_to(
        "fn foo(x: f32) -> f32 { return x; }
         fn foo(x: i32) -> f32 { return foo(f32(x)); }
         fn main() { let a = foo(1i); }",
        "fn foo_f32(x: f32) -> f32 { return x; }
         fn foo_i32(x: i32) -> f32 { return foo_f32(f32(x)); }
         fn main() { let a = foo_i32(1i); }",
    );
}

#[test]
fn local_and_global_scope() {
    assert_compiles_to(
        "struct S { x: f32 }
         var<private> s: S;
         override o: u32;
         const c = 1i;
         fn foo(x: S) {}
         fn foo(x: u32) {}
         fn foo(x: i32) {}
         fn foo(x: ptr<function, f32>) {}
         fn main() {
             var v = 1.0;
             foo(s); foo(o); foo(c); foo(&v);
             for (var i = 0u; i < 10; i++) { foo(i); }
         }",
        "struct S { x: f32 }
         var<private> s: S;
         override o: u32;
         const c = 1i;
         fn foo_S(x: S) {}
         fn foo_u32(x: u32) {}
         fn foo_i32(x: i32) {}
         fn foo_ptr_function_f32_read_write(x: ptr<function, f32>) {}
         fn main() {
             var v = 1.0;
             foo_S(s); foo_u32(o); foo_i32(c); foo_ptr_function_f32_read_write(&v);
             for (var i = 0u; i < 10; i++) { foo_u32(i); }
         }",
    );
}

#[test]
fn const_initializer() {
    assert_compiles_to(
        "fn foo(x: f32) -> f32 { return x * 2.0; }
         fn foo(x: i32) -> i32 { return x * 2; }
         const a = foo(1i);
         const b = foo(1.0);",
        "fn foo_f32(x: f32) -> f32 { return x * 2.0; }
         fn foo_i32(x: i32) -> i32 { return x * 2; }
         const a = foo_i32(1i);
         const b = foo_f32(1.0);",
    );
}

#[test]
fn lower_const_eval() {
    let options = CompileOptions {
        strip: Some(StrippingOptions::KeepMainModule),
        lower: true,
        ..Default::default()
    };
    let res = compile(
        "fn foo(x: f32) -> f32 { return x * 2.0; }
         fn foo(x: i32) -> i32 { return x * 3; }
         const_assert foo(2) == 6;
         const_assert foo(2.0) == 4.0;
         fn main() -> f32 { return foo(1.0); }",
        &[],
        options,
    )
    .inspect_err(|e| eprintln!("{e}"))
    .unwrap();
    assert!(res.contains("foo_f32"), "{res}");
}

#[test]
fn alias_param() {
    assert_fails(
        "alias T = f32;
         fn foo(x: T) {}
         fn foo(x: f32) {}",
        "same parameter types",
    );
}

#[test]
fn duplicate_signature() {
    assert_fails(
        "fn foo(x: f32) -> f32 { return x; }
         fn foo(x: f32) -> i32 { return 1; }",
        "same parameter types",
    );
}

#[test]
fn no_matching_overload() {
    assert_fails(
        "fn foo(x: f32) {}
         fn foo(x: i32) {}
         fn main() { foo(1u); }",
        "no overload",
    );
}

#[test]
fn ambiguous() {
    assert_fails(
        "fn foo(x: f32, y: i32) {}
         fn foo(x: i32, y: f32) {}
         fn main() { foo(1, 1); }",
        "ambiguous",
    );
}

#[test]
fn not_a_function() {
    assert_fails(
        "fn foo(x: f32) {}
         const foo = 1;",
        "duplicate declaration",
    );
}

#[test]
fn entry_point() {
    assert_fails(
        "@compute @workgroup_size(1) fn main() {}
         fn main(x: f32) {}",
        "cannot be overloaded",
    );
}

#[test]
fn visibility() {
    assert_fails(
        "public fn foo(x: f32) {}
         private fn foo(x: i32) {}",
        "same visibility",
    );
}

#[test]
fn recursion() {
    assert_fails(
        "fn foo(x: f32) { foo(1.0); }
         fn foo(x: i32) {}",
        "cyclic",
    );
}

#[test]
fn imported_overloads() {
    let options = CompileOptions::default();
    let res = compile(
        "import package::util::foo;
         @fragment fn main() -> @location(0) vec4f { return vec4f(foo(1.0), foo(1i), 0.0, 1.0); }",
        &[(
            "package::util",
            "fn foo(x: f32) -> f32 { return bar(x); }
             fn foo(x: i32) -> f32 { return baz(x); }
             fn foo(x: u32) -> f32 { return qux(x); }
             fn bar(x: f32) -> f32 { return x; }
             fn baz(x: i32) -> f32 { return f32(x); }
             fn qux(x: u32) -> f32 { return f32(x); }",
        )],
        options,
    )
    .inspect_err(|e| eprintln!("{e}"))
    .unwrap();
    // the dependencies of all overloads are analyzed, but unused overloads are stripped.
    assert!(res.contains("package_util_foo_f32"), "{res}");
    assert!(res.contains("package_util_foo_i32"), "{res}");
    assert!(!res.contains("package_util_foo_u32"), "{res}");
    assert!(res.contains("package_util_baz"), "{res}");
}

#[test]
fn overload_across_modules_is_duplicate() {
    let res = compile(
        "import package::util::foo;
         fn foo(x: i32) {}",
        &[("package::util", "fn foo(x: f32) {}")],
        CompileOptions::default(),
    );
    assert!(res.is_err_and(|e| e.contains("duplicate declaration")));
}
