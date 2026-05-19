//! wesl.toml configuration parsing and file scanning.
//!
//! This module handles reading wesl.toml configuration files and building
//! module hierarchies from glob patterns.
//!
//! ```toml
//! # Version of WESL used in this project.
//! edition = "2026_pre"
//!
//! # Optional, can be auto-inferred from the existence of a package manager file.
//! # Inclusion of this field is encouraged.
//! package-manager = "npm"
//!
//! # Where are the shaders located. This is the path of `package::`.
//! root = "./shaders"
//!
//! # Optional
//! include = [ "shaders/**/*.wesl", "shaders/**/*.wgsl" ]
//!
//! # Optional.
//! # Some projects have large folders that we shouldn't react to.
//! exclude = [ "**/test" ]
//!
//! # Lists all used dependencies
//! [dependencies]
//! # Shorthand for `foolib = { package = "foolib" }`
//! foolib = {}
//! # Can be used for renaming packages. Now bevy in my code is called "cute_bevy".
//! cute_bevy = { package = "bevy" }
//! # File path to a folder with a wesl.toml. Simplest kind of dependency.
//! mylib = { path = "../mylib" }
//! ```

//!

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::package::{Module, RESERVED_MOD_NAMES, is_mod_ident};

/// Parsed wesl.toml configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeslToml {
    /// Package configuration.
    #[serde(flatten)]
    package: PackageConfig,
    /// The `[dependencies]` section.
    #[serde(default)]
    dependencies: DependenciesConfig,
}

/// Package configuration fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageConfig {
    /// WESL edition (required).
    edition: WeslEdition,
    /// Package manager: "npm" or "cargo". Auto-detected if not specified.
    // TODO: auto-detect is not implemented, it just defaults to cargo.
    #[serde(default)]
    package_manager: PackageManager,
    /// Root folder for package:: syntax. Default: "./shaders/"
    #[serde(default = "default_root")]
    root: PathBuf,
    /// Glob patterns for files to include. Default: all .wesl/.wgsl in root.
    #[serde(default = "default_include")]
    include: Vec<String>,
    /// Glob patterns for files to exclude. Default: empty.
    #[serde(default = "default_exclude")]
    exclude: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename = "snake_case")]
pub enum WeslEdition {
    #[serde(rename = "2026_pre")]
    Unstable2026,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageManager {
    // TODO: the spec says: "can be inferred from the existence of certain files"
    #[default]
    Cargo,
    Npm,
}

fn default_root() -> PathBuf {
    PathBuf::from("./shaders/")
}
fn default_include() -> Vec<String> {
    vec!["**/*.wesl".to_string(), "**/*.wgsl".to_string()]
}
fn default_exclude() -> Vec<String> {
    vec!["**/node_modules/".to_string()]
}

/// The [dependencies] section of wesl.toml.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(
    untagged,
    try_from = "DependenciesConfigProxy",
    into = "DependenciesConfigProxy"
)]
pub enum DependenciesConfig {
    /// No dependencies specified.
    #[default]
    None,
    /// Automatic dependency detection: `dependencies = "auto"`
    Auto,
    /// Explicit dependency table.
    Manual(HashMap<String, DependencySpec>),
}

/// Intermediate data type for serialization / deserialization
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum DependenciesConfigProxy {
    None,
    Auto(String),
    Manual(HashMap<String, DependencySpec>),
}

impl TryFrom<DependenciesConfigProxy> for DependenciesConfig {
    type Error = ScanTomlError;

    fn try_from(cfg: DependenciesConfigProxy) -> Result<Self, Self::Error> {
        match cfg {
            DependenciesConfigProxy::None => Ok(DependenciesConfig::None),
            DependenciesConfigProxy::Auto(s) if s == "auto" => Ok(DependenciesConfig::Auto),
            DependenciesConfigProxy::Auto(_) => Err(ScanTomlError::ExpectedAuto),
            DependenciesConfigProxy::Manual(map) => Ok(DependenciesConfig::Manual(map)),
        }
    }
}

impl From<DependenciesConfig> for DependenciesConfigProxy {
    fn from(dep: DependenciesConfig) -> Self {
        match dep {
            DependenciesConfig::None => DependenciesConfigProxy::None,
            DependenciesConfig::Auto => DependenciesConfigProxy::Auto("auto".into()),
            DependenciesConfig::Manual(map) => DependenciesConfigProxy::Manual(map),
        }
    }
}

/// A single dependency specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    untagged,
    try_from = "DependencySpecProxy",
    into = "DependencySpecProxy"
)]
pub enum DependencySpec {
    /// Auto: `mydep = { }`
    Auto,
    /// Package name (for renaming): `mydep = { package = "actual_name" }`
    Package(String),
    /// Local path: `mydep = { path = "../lib" }`
    Path(String),
}

/// Intermediate data type for serialization / deserialization
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencySpecProxy {
    Package { package: String },
    Path { path: String },
    // note, this one has to come last so the other two take priority.
    Auto {},
}

impl From<DependencySpecProxy> for DependencySpec {
    fn from(cfg: DependencySpecProxy) -> Self {
        match cfg {
            DependencySpecProxy::Auto {} => DependencySpec::Auto,
            DependencySpecProxy::Package { package } => DependencySpec::Package(package),
            DependencySpecProxy::Path { path } => DependencySpec::Path(path),
        }
    }
}

impl From<DependencySpec> for DependencySpecProxy {
    fn from(dep: DependencySpec) -> Self {
        match dep {
            DependencySpec::Auto => DependencySpecProxy::Auto {},
            DependencySpec::Package(package) => DependencySpecProxy::Package { package },
            DependencySpec::Path(path) => DependencySpecProxy::Path { path },
        }
    }
}

/// Warning emitted during file scanning (non-fatal).
#[derive(Debug, Clone)]
pub enum ScanWarning {
    /// A path component is not a valid WGSL identifier; file was skipped.
    InvalidIdentifier { component: String, file: PathBuf },
    /// A path component is a reserved module name; file was skipped.
    ReservedModName { name: String, file: PathBuf },
}

impl std::fmt::Display for ScanWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentifier { component, file } => {
                write!(
                    f,
                    "skipped file: component `{component}` is not a valid WGSL identifier in {file:?}"
                )
            }
            Self::ReservedModName { name, file } => {
                write!(
                    f,
                    "skipped file: module name `{name}` is reserved in {file:?}"
                )
            }
        }
    }
}

/// Result of scanning files, including any non-fatal warnings.
#[derive(Debug)]
pub struct ScanResult {
    /// The root module containing the scanned file hierarchy.
    pub module: Module,
    /// Warnings encountered during scanning.
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanTomlError {
    #[error("wesl.toml not found at `{0}`")]
    TomlNotFound(PathBuf),
    #[error("Failed to parse wesl.toml: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("expected dependencies = \"auto\"")]
    ExpectedAuto,
    #[error("Invalid glob pattern `{0}`: {1}")]
    InvalidGlob(String, glob::PatternError),
    #[error("File `{0}` is outside root `{1}`")]
    FileOutsideRoot(PathBuf, PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No source files matched the include patterns")]
    NoFilesMatched,
    #[error("Multiple files map to module `{0}`: {1:?}")]
    ConflictingFiles(String, Vec<PathBuf>),
}

impl WeslToml {
    /// Parse a wesl.toml file from a path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ScanTomlError> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::parse_str(&content)
    }

    /// Parse a wesl.toml from string content.
    pub fn parse_str(content: &str) -> Result<Self, ScanTomlError> {
        toml::from_str(content).map_err(ScanTomlError::TomlParse)
    }
}

/// Scan files based on a WeslToml and build a Module hierarchy.
///
/// This is the main entry point called by `PkgBuilder::scan_toml`.
pub fn scan_from_config(
    name: &str,
    base_dir: &Path,
    config: &WeslToml,
) -> Result<ScanResult, ScanTomlError> {
    let root_path = std::path::absolute(base_dir.join(&config.package.root))?;

    let include = compile_patterns(&config.package.include)?;
    let exclude = compile_patterns(&config.package.exclude)?;

    let matched_files = walk_directory(base_dir, &root_path, &include, &exclude)?;

    if matched_files.is_empty() {
        return Err(ScanTomlError::NoFilesMatched);
    }

    let (module, warnings) = build_module_hierarchy(name, &matched_files, &root_path)?;
    Ok(ScanResult { module, warnings })
}

/// Compile glob pattern strings into `glob::Pattern` values.
fn compile_patterns(patterns: &[String]) -> Result<Vec<glob::Pattern>, ScanTomlError> {
    patterns
        .iter()
        .map(|p| {
            let stripped = p.strip_prefix("./").unwrap_or(p);
            glob::Pattern::new(stripped).map_err(|e| ScanTomlError::InvalidGlob(p.clone(), e))
        })
        .collect()
}

/// Walk `root_dir` recursively, collecting files that match any include pattern
/// and no exclude pattern. Patterns are matched against paths relative to
/// `base_dir` (the directory containing wesl.toml). Directories containing
/// their own `wesl.toml` are treated as separate packages and skipped entirely.
fn walk_directory(
    base_dir: &Path,
    root_dir: &Path,
    include: &[glob::Pattern],
    exclude: &[glob::Pattern],
) -> Result<HashSet<PathBuf>, ScanTomlError> {
    let mut files = HashSet::new();
    let mut stack = vec![root_dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => return Err(ScanTomlError::Io(e)),
        };

        for entry in entries {
            let path = entry?.path();
            let rel = path.strip_prefix(base_dir).unwrap_or(&path);

            if glob_match(exclude, rel) {
                // skip excluded paths
            } else if path.is_dir() {
                let is_nested_pkg = path != root_dir && path.join("wesl.toml").is_file();
                if !is_nested_pkg {
                    stack.push(path);
                }
            } else if path.is_file() && glob_match(include, rel) {
                files.insert(path);
            }
        }
    }

    Ok(files)
}

/// Check if a path matches any of the given glob patterns.
fn glob_match(patterns: &[glob::Pattern], path: &Path) -> bool {
    let opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };
    patterns.iter().any(|pat| pat.matches_path_with(path, opts))
}

struct FileEntry {
    path: PathBuf,
    module_components: Vec<String>,
}

/// Build a Module hierarchy from a flat list of files.
fn build_module_hierarchy(
    root_name: &str,
    files: &HashSet<PathBuf>,
    root_path: &Path,
) -> Result<(Module, Vec<ScanWarning>), ScanTomlError> {
    let (entries, warnings) = derive_module_paths(files, root_path)?;
    let module = build_module_tree(root_name, entries)?;
    Ok((module, warnings))
}

/// Extract module path components from a relative file path.
///
/// Combines the parent directory components with the file stem.
fn path_to_components(relative: &Path) -> Vec<String> {
    let mut components = Vec::new();
    if let Some(parent) = relative.parent() {
        components.extend(parent.iter().map(|c| c.to_string_lossy().to_string()));
    }
    if let Some(stem) = relative.file_stem() {
        components.push(stem.to_string_lossy().to_string());
    }
    components
}

/// Validate module path components.
///
/// Returns `None` if valid, or `Some(warning)` if the file should be skipped.
fn validate_components(components: &[String], file_path: &Path) -> Option<ScanWarning> {
    for comp in components {
        if RESERVED_MOD_NAMES.contains(&comp.as_str()) {
            return Some(ScanWarning::ReservedModName {
                name: comp.clone(),
                file: file_path.to_path_buf(),
            });
        }
        if !is_mod_ident(comp) {
            return Some(ScanWarning::InvalidIdentifier {
                component: comp.clone(),
                file: file_path.to_path_buf(),
            });
        }
    }
    None
}

/// Derive module path components from file paths by stripping the root prefix.
fn derive_module_paths(
    files: &HashSet<PathBuf>,
    root_path: &Path,
) -> Result<(Vec<FileEntry>, Vec<ScanWarning>), ScanTomlError> {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for file_path in files {
        let relative = file_path.strip_prefix(root_path).map_err(|_| {
            ScanTomlError::FileOutsideRoot(file_path.clone(), root_path.to_path_buf())
        })?;

        let components = path_to_components(relative);

        if !components.is_empty() {
            if let Some(warning) = validate_components(&components, file_path) {
                warnings.push(warning);
            } else {
                entries.push(FileEntry {
                    path: file_path.clone(),
                    module_components: components,
                });
            }
        }
    }

    Ok((entries, warnings))
}

/// Intermediate tree node used while building the module hierarchy.
///
/// The `path` field tracks which file was assigned to detect conflicts
/// (e.g., both `main.wesl` and `main.wgsl` mapping to the same module).
struct ModuleNode {
    path: Option<PathBuf>,
    source: String,
    children: HashMap<String, ModuleNode>,
}

impl ModuleNode {
    fn new() -> Self {
        Self {
            path: None,
            source: String::new(),
            children: HashMap::new(),
        }
    }

    fn into_module(self, name: String) -> Module {
        let submodules = self
            .children
            .into_iter()
            .map(|(name, node)| node.into_module(name))
            .collect();

        Module {
            name,
            source: self.source,
            submodules,
        }
    }
}

/// Build a tree of Modules from flat file entries.
fn build_module_tree(root_name: &str, entries: Vec<FileEntry>) -> Result<Module, ScanTomlError> {
    let mut root = ModuleNode::new();

    for entry in entries {
        let Some((leaf, parents)) = entry.module_components.split_last() else {
            continue;
        };

        // Traverse/create intermediate nodes for parent components
        let mut current = &mut root;
        for comp in parents {
            current = current
                .children
                .entry(comp.clone())
                .or_insert_with(ModuleNode::new);
        }

        // Create/update the leaf node for the actual module
        let node = current
            .children
            .entry(leaf.clone())
            .or_insert_with(ModuleNode::new);

        if let Some(existing_path) = &node.path {
            let module_name = entry.module_components.join("::");
            return Err(ScanTomlError::ConflictingFiles(
                module_name,
                vec![existing_path.clone(), entry.path.clone()],
            ));
        }

        node.path = Some(entry.path.clone());
        node.source = std::fs::read_to_string(&entry.path).map_err(ScanTomlError::Io)?;
    }

    Ok(root.into_module(root_name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixtures_dir() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/wesl_toml"
        ))
    }

    #[test]
    fn parse_example_toml() {
        let toml_str = r#"
            edition = "2026_pre"
            package_manager = "npm"
            root = "./shaders"
            include = [ "shaders/**/*.wesl", "shaders/**/*.wgsl" ]
            exclude = [ "**/test" ]

            [dependencies]
            foolib = {}
            cute_bevy = { package = "bevy" }
            mylib = { path = "../mylib" }

            [package]
        "#;

        let parsed: WeslToml = toml::from_str(toml_str).unwrap();

        assert_eq!(parsed.package.edition, WeslEdition::Unstable2026);
        assert_eq!(parsed.package.package_manager, PackageManager::Npm);
        assert_eq!(parsed.package.root, PathBuf::from("./shaders"));
        assert!(
            parsed
                .package
                .include
                .contains(&"shaders/**/*.wesl".to_string())
        );

        match parsed.dependencies {
            DependenciesConfig::Manual(deps) => {
                assert!(matches!(deps.get("foolib").unwrap(), DependencySpec::Auto));
                println!("x {:?}", deps);
                println!("x {:?}", deps.get("cute_bevy").unwrap());
                assert!(matches!(
                    deps.get("cute_bevy").unwrap(),
                    DependencySpec::Package(_)
                ));
                assert!(matches!(
                    deps.get("mylib").unwrap(),
                    DependencySpec::Path(_)
                ));
            }
            _ => panic!("expected manual dependencies"),
        }
    }

    #[test]
    fn test_config_parsing() {
        // Basic config with defaults
        let basic = WeslToml::parse_str("edition = \"2026_pre\"").unwrap();
        assert_eq!(basic.package.edition, WeslEdition::Unstable2026);
        assert_eq!(basic.package.root, default_root());
        assert_eq!(basic.package.include, default_include());
        assert_eq!(basic.package.exclude, default_exclude());

        // Config with custom root
        let with_root = WeslToml::parse_str("edition = \"2026_pre\"\nroot = \"./src/\"").unwrap();
        assert_eq!(with_root.package.edition, WeslEdition::Unstable2026);
        assert_eq!(with_root.package.root, Path::new("./src/"));

        // Explicit empty exclude overrides default
        let no_exclude = WeslToml::parse_str("edition = \"2026_pre\"\nexclude = []").unwrap();
        assert!(no_exclude.package.exclude.is_empty());

        // Missing edition
        let missing = WeslToml::parse_str("root = \"./shaders/\"");
        assert!(matches!(missing, Err(ScanTomlError::TomlParse(_))));

        // Dependencies variants
        let with_deps =
            WeslToml::parse_str("edition = \"2026_pre\"\n\n[dependencies]\nfoo = {}").unwrap();
        assert!(matches!(
            with_deps.dependencies,
            DependenciesConfig::Manual(_)
        ));

        let auto_deps =
            WeslToml::parse_str("edition = \"2026_pre\"\ndependencies = \"auto\"").unwrap();
        assert!(matches!(auto_deps.dependencies, DependenciesConfig::Auto));
    }

    #[test]
    fn test_scan_from_config() {
        let base = fixtures_dir().join("basic");
        let config = WeslToml::parse_str("edition = \"2026_pre\"\nroot = \"./shaders/\"").unwrap();
        let result = scan_from_config("my_pkg", &base, &config).unwrap();

        assert_eq!(result.module.name, "my_pkg");
        assert_eq!(result.module.submodules.len(), 2);
        assert!(result.warnings.is_empty());

        let main_mod = result
            .module
            .submodules
            .iter()
            .find(|m| m.name == "main")
            .unwrap();
        assert_eq!(main_mod.source.trim(), "// main");

        let utils = result
            .module
            .submodules
            .iter()
            .find(|m| m.name == "utils")
            .unwrap();
        assert_eq!(utils.submodules[0].name, "math");
    }

    #[test]
    fn test_conflicting_files_error() {
        let base = fixtures_dir().join("conflict");
        let config = WeslToml::parse_str("edition = \"2026_pre\"\nroot = \"./shaders/\"").unwrap();
        let result = scan_from_config("my_pkg", &base, &config);

        assert!(matches!(result, Err(ScanTomlError::ConflictingFiles(_, _))));
    }

    #[test]
    fn test_exclude_directory() {
        let base = fixtures_dir().join("exclude");
        let config = WeslToml::parse_str(
            r#"
            edition = "2026_pre"
            root = "./shaders/"
            exclude = ["**/test"]
            "#,
        )
        .unwrap();

        let result = scan_from_config("my_pkg", &base, &config).unwrap();

        // Should only have main, not the test/fixture
        assert_eq!(result.module.submodules.len(), 1);
        assert_eq!(result.module.submodules[0].name, "main");
    }

    #[test]
    fn test_overlapping_patterns_deduplicated() {
        // Overlapping patterns matching the same files should be deduplicated
        let base = fixtures_dir().join("basic");
        let config = WeslToml::parse_str(
            r#"
            edition = "2026_pre"
            root = "./shaders/"
            include = ["shaders/**/*.wesl", "shaders/**/*.wesl"]
            "#,
        )
        .unwrap();
        let result = scan_from_config("my_pkg", &base, &config).unwrap();

        // Should still be 2 modules (main + utils), not duplicated
        assert_eq!(result.module.submodules.len(), 2);
    }

    #[test]
    fn test_nested_wesl_toml_excluded() {
        // A subdirectory with its own wesl.toml should be excluded from scanning
        let base = fixtures_dir().join("nested");
        let config = WeslToml::parse_str(
            r#"
            edition = "2026_pre"
            root = "./shaders/"
            "#,
        )
        .unwrap();
        let result = scan_from_config("my_pkg", &base, &config).unwrap();

        // Should only contain `main`, not `subpkg/inner`
        assert_eq!(result.module.submodules.len(), 1);
        assert_eq!(result.module.submodules[0].name, "main");
    }
}
