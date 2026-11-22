//! Integration tests for the compiler frontend.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use expect_test::expect_file;
use wesl::{
    AsyncResolver, CompileOptions, Compiler, Constants, Features, Resolver, error::ResolveError,
    syntax::ModulePath,
};

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

// like the wesl::wesl_pkg macro, but in the fixtures dir.
macro_rules! wesl_pkg {
    ($pkg_name:ident, $source:expr) => {
        mod $pkg_name {
            use wesl_core::{StaticPackage, StaticPackageModule};
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
    compiler.options.strip = None;
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    let expected = expect_file!["./snapshots/frontend__compile_wgsl.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
}

#[test]
fn compile_wgsl_lower_eval() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/main.wgsl");

    let mut compiler = Compiler::default();

    compiler.options.lower = true;
    compiler.options.strip = None;
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    let expected = expect_file!["./snapshots/frontend__compile_wgsl_lower_eval.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
}

#[test]
fn compile_wgsl_strip() {
    let test_path = fixtures_dir().join("compile_wgsl/shaders/main.wgsl");

    let mut compiler = Compiler::default();

    compiler.options.lower = false;
    compiler.options.strip = Some(wesl::StrippingOptions::KeepPipelineVisible);
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    let expected = expect_file!["./snapshots/frontend__compile_wgsl_strip.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
}

/// same as test `compile_wesl_toml` below, except the path provided is a directory,
/// so it doesn't use the toml file and instead looks for a `package.wesl` file
/// in the directory as both the main module and the root path.
#[test]
fn compile_wesl_directory() {
    let test_path = fixtures_dir().join("compile_wesl/shaders");

    let mut compiler = Compiler::default();

    compiler.options.lower = false;
    compiler.options.strip = None;
    compiler.options.mangle_main = true;
    compiler.options.features.set("constants", true);
    compiler.options.features.set("random", true);

    let mut constants = Constants::new();
    constants.set("PI", std::f64::consts::PI);
    constants.set("TRUE", true);
    compiler.options.constants = constants;

    compiler.options.dependencies = vec![&package_random::PACKAGE];
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    let expected = expect_file!["./snapshots/frontend__compile_wesl_directory.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
}

#[tokio::test]
async fn compile_wesl_async() {
    let test_path = fixtures_dir().join("compile_wesl/shaders");

    let mut compiler = Compiler::default();

    compiler.options.lower = false;
    compiler.options.strip = None;
    compiler.options.mangle_main = true;
    compiler.options.features.set("constants", false);
    compiler.options.features.set("random", false);

    let mut result = compiler
        .with_resolver(TokioFileResolver::new(&test_path))
        .compile_module_async(&ModulePath::new_root())
        .await
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    let expected = expect_file![format!("./snapshots/frontend__compile_wesl_async.snap")];
    expected.assert_eq(&result.syntax.to_string());
}

/// same as test `compile_wesl_toml` below, except the path provided is a file,
/// so it doesn't use the toml file and sets the provided path as the main module,
/// and the parent directory as the root path.
#[test]
fn compile_wesl_file() {
    let test_path = fixtures_dir().join("compile_wesl/shaders/package.wesl");

    let mut compiler = Compiler::default();

    compiler.options.lower = false;
    compiler.options.strip = None;
    compiler.options.mangle_main = true;
    compiler.options.features.set("constants", true);
    compiler.options.features.set("random", true);
    compiler.options.constants.set("PI", std::f64::consts::PI);
    compiler.options.constants.set("TRUE", true);

    compiler.options.dependencies = vec![&package_random::PACKAGE];
    let mut result = compiler
        .compile(&test_path)
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();
    result.syntax.sort_declarations(); // normalize for comparison
    let expected = expect_file!["./snapshots/frontend__compile_wesl_file.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
}

#[test]
fn compile_wesl_toml_feat1() {
    let test_path = fixtures_dir().join("compile_wesl/wesl.toml");

    let mut features = Features::new();
    features.set("feat1", true);
    features.set("feat2", false);
    features.set("constants", true);
    features.set("random", true);
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
    let expected = expect_file!["./snapshots/frontend__compile_wesl_toml_feat1.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
}

#[test]
fn compile_wesl_toml_feat2() {
    let test_path = fixtures_dir().join("compile_wesl/wesl.toml");

    let mut features = Features::new();
    features.set("feat1", false);
    features.set("feat2", true);
    features.set("constants", true);
    features.set("random", true);
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
    let expected = expect_file!["./snapshots/frontend__compile_wesl_toml_feat2.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
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
    let expected =
        expect_file!["./snapshots/frontend__compile_dependency_unification.snap".to_string()];
    expected.assert_eq(&result.syntax.to_string());
}

/// Mirrors [`wesl::resolver::FileResolver`] but uses tokio for async file loading.
pub struct TokioFileResolver {
    pkg_root_dir: PathBuf,
    extension: &'static str,
}

impl TokioFileResolver {
    pub fn new(pkg_root_dir: impl AsRef<Path>) -> Self {
        Self {
            pkg_root_dir: pkg_root_dir.as_ref().to_path_buf(),
            extension: "wesl",
        }
    }

    fn file_path(&self, path: &ModulePath) -> Result<PathBuf, ResolveError> {
        if path.origin.is_package() {
            return Err(ResolveError::ModuleNotFound(
                path.clone(),
                "this is an external package import, not a file import. Use `package::` or `super::` for file imports."
                    .to_string(),
            ));
        }
        let mut fs_path = self.pkg_root_dir.to_path_buf();
        fs_path.extend(&path.components);
        if path.is_root() {
            // the root module path (package) corresponds to the special file `package.wesl` in the root directory.
            fs_path.push("package");
        }
        fs_path.set_extension(self.extension);
        if fs_path.exists() {
            Ok(fs_path)
        } else {
            let wgsl_fs_path = fs_path.with_extension("wgsl");
            if wgsl_fs_path.exists() {
                Ok(wgsl_fs_path)
            } else {
                Err(ResolveError::FileNotFound(
                    fs_path,
                    "physical file".to_string(),
                ))
            }
        }
    }
}

impl Resolver for TokioFileResolver {
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        self.file_path(path)
            .ok()
            .map(|fs_path| fs_path.display().to_string())
    }
    fn fs_path(&self, path: &ModulePath) -> Result<PathBuf, ResolveError> {
        self.file_path(path)
    }
    fn module_path(&self, fs_path: &Path) -> Result<ModulePath, ResolveError> {
        let abs_root = std::path::absolute(&self.pkg_root_dir).map_err(ResolveError::Io)?;
        let abs_path = std::path::absolute(fs_path).map_err(ResolveError::Io)?;
        let rel_path = abs_path
            .strip_prefix(&abs_root)
            .map_err(|_| ResolveError::FileEscapesRoot(abs_path.clone(), abs_root))?;

        // TODO: from_path can panic, we don't want that here.
        Ok(ModulePath::from_path(rel_path))
    }
}

impl AsyncResolver for TokioFileResolver {
    async fn resolve_source_async<'a>(
        &'a self,
        path: &ModulePath,
    ) -> Result<Cow<'a, str>, ResolveError> {
        let fs_path = self.file_path(path)?;
        let source = tokio::fs::read_to_string(&fs_path)
            .await
            .map_err(|_| ResolveError::FileNotFound(fs_path, "physical file".to_string()))?;

        Ok(source.into())
    }
}
