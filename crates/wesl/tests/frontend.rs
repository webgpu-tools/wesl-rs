//! Integration tests for the compiler frontend.

use std::path::Path;

use wesl::{Feature, Wesl};

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

// like the wesl::wesl_pkg macro, but in the fixtures dir.
macro_rules! wesl_pkg {
    ($pkg_name:ident, $source:expr) => {
        mod $pkg_name {
            use wesl::{CodegenModule, CodegenPkg};
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
    let test_path = fixtures_dir().join("compile_wgsl/shaders/");

    let mut compiler = Wesl::new(test_path);
    compiler.use_lower(false).use_stripping(false);

    let mut result = compiler
        .compile(&"package::main".parse().unwrap())
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[cfg(not(feature = "eval"))]
#[test]
fn compile_wgsl_lower() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/");

    let mut compiler = Wesl::new(test_path);
    compiler.use_lower(true).use_stripping(false);

    let mut result = compiler
        .compile(&"package::main".parse().unwrap())
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[cfg(feature = "eval")]
#[test]
fn compile_wgsl_lower_eval() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/");

    let mut compiler = Wesl::new(test_path);
    compiler.use_lower(true).use_stripping(false);

    let mut result = compiler
        .compile(&"package::main".parse().unwrap())
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[test]
fn compile_wgsl_strip() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/");

    let mut compiler = Wesl::new(test_path);
    compiler.use_lower(false).use_stripping(true);

    let mut result = compiler
        .compile(&"package::main".parse().unwrap())
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[test]
#[ignore = "wesl.toml not implemented for the compiler"]
fn compile_wesl_toml_feat1() {
    let test_path = fixtures_dir().join("compile_wesl/wesl.toml");

    let mut result = Wesl::new(test_path)
        .set_missing_feature_behavior(Feature::Error)
        .set_features([("feat1", true), ("feat2", false)])
        .add_constants([("PI", std::f64::consts::PI.into()), ("TRUE", true.into())])
        .add_package(&package_random::PACKAGE)
        .compile(&"package::main".parse().unwrap())
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}

#[test]
#[ignore = "wesl.toml not implemented for the compiler"]
fn compile_wesl_toml_feat2() {
    let test_path = fixtures_dir().join("compile_wesl/wesl.toml");

    let mut result = Wesl::new(test_path)
        .set_missing_feature_behavior(Feature::Error)
        .set_features([("feat1", true), ("feat2", false)])
        .add_constants([("PI", std::f64::consts::PI.into()), ("TRUE", true.into())])
        .add_package(&package_random::PACKAGE)
        .compile(&"package::main".parse().unwrap())
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
    let test_path = fixtures_dir().join("dependency_unification");

    // in this test, A imports from C1 and D1, B imports from
    let mut result = Wesl::new(test_path)
        .add_packages([&a::PACKAGE, &b::PACKAGE])
        .compile(&"package::main".parse().unwrap())
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    insta::assert_snapshot!(result.syntax.to_string());
}
