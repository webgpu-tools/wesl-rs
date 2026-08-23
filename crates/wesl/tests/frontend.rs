//! Integration tests for the compiler frontend.

use std::path::Path;

use wesl::{CompileOptions, Compiler, Constants, Features};

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

// like the wesl::wesl_pkg macro, but in the fixtures dir.
macro_rules! wesl_pkg {
    ($pkg_name:ident, $source:expr) => {
        mod $pkg_name {
            use wesl::package::{StaticPackage, StaticPackageModule};
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/",
                $source,
            ));
        }
    };
}

wesl_pkg!(package_random, "package_random.rs");

#[test]
fn compile_wgsl() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/main.wgsl");

    let mut compiler = Compiler::default();

    compiler.options.lower = false;
    compiler.options.strip = false;
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[tokio::test]
async fn compile_wgsl_async() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/main.wgsl");

    let mut compiler = Compiler::default();

    compiler.options.lower = false;
    compiler.options.strip = false;
    let mut result = compiler
        .compile_async(&test_path)
        .await
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[cfg(not(feature = "eval"))]
#[test]
fn compile_wgsl_lower() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/main.wgsl");

    let mut compiler = Compiler::default();

    compiler.options.lower = true;
    compiler.options.strip = false;
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[cfg(feature = "eval")]
#[test]
fn compile_wgsl_lower_eval() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/main.wgsl");

    let mut compiler = Compiler::default();

    compiler.options.lower = true;
    compiler.options.strip = false;
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[test]
fn compile_wgsl_strip() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/main.wgsl");

    let mut compiler = Compiler::default();

    compiler.options.lower = false;
    compiler.options.strip = true;
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[test]
fn compile_wesl_toml_feat1() {
    let test_path = fixtures_dir().join("compile_wesl/wesl.toml");

    let mut features = Features::new();
    features.set("feat1", true);
    features.set("feat2", false);
    features.default = wesl::Feature::Error;

    let mut constants = Constants::new();
    constants.set("PI", std::f64::consts::PI);
    constants.set("TRUE", true);

    let mut result = Compiler::new(CompileOptions {
        features,
        constants,
        dependencies: vec![&package_random::PACKAGE],
        ..Default::default()
    })
    .compile(&test_path)
    .inspect_err(|e| eprintln!("{e}"))
    .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[test]
fn compile_wesl_toml_feat2() {
    let test_path = fixtures_dir().join("compile_wesl/wesl.toml");

    let mut features = Features::new();
    features.set("feat1", false);
    features.set("feat2", true);
    features.default = wesl::Feature::Error;

    let mut constants = Constants::new();
    constants.set("PI", std::f64::consts::PI);
    constants.set("TRUE", true);

    let mut result = Compiler::new(CompileOptions {
        features,
        constants,
        dependencies: vec![&package_random::PACKAGE],
        ..Default::default()
    })
    .compile(&test_path)
    .inspect_err(|e| eprintln!("{e}"))
    .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

wesl_pkg!(a, "dependency_unification/a.rs");
wesl_pkg!(b, "dependency_unification/b.rs");
// C1 is never used (unified with C2 which is semver-compatible with higher version)
wesl_pkg!(c1, "dependency_unification/c1.rs");
wesl_pkg!(c2, "dependency_unification/c2.rs");
wesl_pkg!(d1, "dependency_unification/d1.rs");
wesl_pkg!(d2, "dependency_unification/d2.rs");

/// This test was taken from examples/dependency-resolution.
///
/// A imports from C1 and D1, B imports from C2 and D2.
/// But C1 is semver-compatible with C2, so it is replaced with C2.
/// This test asserts that references to C items from A and B have the same name,
/// and used C items are declared exactly once.
#[test]
fn compile_dependency_unification() {
    let test_path = fixtures_dir().join("dependency_unification/main.wesl");

    // in this test, A imports from C1 and D1, B imports from
    let mut result = Compiler::new(CompileOptions {
        dependencies: vec![&a::PACKAGE, &b::PACKAGE],
        ..Default::default()
    })
    .compile(&test_path)
    .inspect_err(|e| eprintln!("{e}"))
    .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}
