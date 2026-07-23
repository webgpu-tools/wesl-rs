//! Integration tests for the compiler frontend.

use std::path::Path;

use wesl::{CompileOptions, Compiler, Constants, Features};

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

// basically an expansion of the `wesl_pkg` macro.
mod package_random {
    use wesl::package::{StaticPackage, StaticPackageModule};
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/package_random.rs"
    ));
}

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
    features.add_feature("feat1", true);
    features.add_feature("feat2", false);
    features.default = wesl::Feature::Error;

    let mut constants = Constants::new();
    constants.add_constant("PI", 3.1415);
    constants.add_constant("TRUE", true);

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
    features.add_feature("feat1", false);
    features.add_feature("feat2", true);
    features.default = wesl::Feature::Error;

    let mut constants = Constants::new();
    constants.add_constant("PI", 3.1415);
    constants.add_constant("TRUE", true);

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
