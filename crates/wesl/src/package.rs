use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::{
    Diagnostic, Error, ModulePath,
    pass::{retarget_idents, validate_wesl},
    resolver::CodegenPkg,
    wesl_toml::{self, ScanTomlError, WeslToml},
};
use quote::{format_ident, quote};
use wgsl_parse::{
    lexer::{Lexer, Token},
    syntax::TranslationUnit,
};
use wgsl_types::idents::RESERVED_WORDS;

/// WGSL identifier predicate, including reserved words, but excluding keywords.
pub(crate) fn is_mod_ident(name: &str) -> bool {
    let mut lex = Lexer::new(name);
    RESERVED_WORDS.contains(&name)
        || matches!(
            (lex.next(), lex.next()),
            (Some(Ok((_, Token::Ident(_), _))), None)
        )
}

// https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/Imports.md#reference-level-explanation
pub(crate) const RESERVED_MOD_NAMES: &[&str] = &[
    // WGSL keywords
    "const_assert",
    "continue",
    "continuing",
    "default",
    "diagnostic",
    "discard",
    "else",
    "enable",
    "false",
    "fn",
    "for",
    "if",
    "let",
    "loop",
    "override",
    "requires",
    "return",
    "struct",
    "switch",
    "true",
    "var",
    "while",
    // WESL keywords
    "self",
    "super",
    "package",
    "as",
    "import",
];

/// A builder that generates code for WESL packages.
///
/// WESL packages are bundles of shader files that can be reused in other projects, like
/// Rust crates. See the `consumer` example to see how to use them.
///
/// This type is mainly designed to be used in a build script (`build.rs` file). Add `wesl`
/// to the build-dependencies of your project and enable the `package` feature flag.
///
/// # Usage
///
/// ```ignore
/// // in build.rs
/// fn main() {
///    wesl::PkgBuilder::new("my_package")
///        // read all wesl files in the directory "src/shaders"
///        .scan_root("src/shaders")
///        .expect("failed to scan WESL files")
///        // validation is optional
///        .validate()
///        .map_err(|e| eprintln!("{e}"))
///        .expect("validation error")
///        // write "my_package.rs" in OUT_DIR
///        .build_artifact()
///        .expect("failed to build artifact");
/// }
/// ```
/// Then, in your `lib.rs` file, expose the generated module with the [`crate::wesl_pkg`] macro.
/// ```ignore
/// wesl::wesl_pkg!(pub my_package);
/// ```
///
/// The package name must be a valid rust identifier, E.g. it must not contain dashes `-`.
/// Dashes are replaced with underscores `_`.
pub struct PkgBuilder {
    name: String,
    dependencies: Vec<&'static CodegenPkg>,
}

/// The type holding the source code of packages.
///
/// This struct is produced by [`PkgBuilder::scan_root`], but one can also create or edit
/// packages manually by modifying this struct. The final package is produced by calling
/// [`Self::build_artifact`] or [`Self::codegen`].
pub struct Pkg {
    pub crate_name: String,
    pub root: Module,
    pub dependencies: Vec<&'static CodegenPkg>,
}

/// The type holding the source code of individual modules in packages.
///
/// See [`Pkg`].
#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub source: String,
    pub submodules: Vec<Module>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanDirectoryError {
    #[error("Package root was not found: `{0}`")]
    RootNotFound(PathBuf),
    #[error("Module name `{0}` is reserved")]
    ReservedModName(String),
    #[error("I/O error while scanning package root: {0}")]
    Io(#[from] std::io::Error),
}

fn cargo_crate_name() -> String {
    std::env::var("CARGO_PKG_NAME").expect("CARGO_PKG_NAME environment variable is not defined")
}

impl PkgBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.replace('-', "_"),
            dependencies: Vec::new(),
        }
    }

    /// Add a package dependency.
    ///
    /// Learn more about packages in [`PkgBuilder`].
    pub fn add_package(mut self, pkg: &'static CodegenPkg) -> Self {
        self.dependencies.push(pkg);
        self
    }

    /// Add several package dependencies.
    ///
    /// Learn more about packages in [`PkgBuilder`].
    pub fn add_packages(mut self, pkgs: impl IntoIterator<Item = &'static CodegenPkg>) -> Self {
        for pkg in pkgs {
            self = self.add_package(pkg);
        }
        self
    }

    /// Reads all files to include in the package, starting from the root module.
    ///
    /// The input path must point at the root file or folder. The package will include
    /// all .wesl and .wgsl files reachable from the root module, recursively.
    /// The name or the root file is ignored, instead the name of the package is used.
    pub fn scan_root(self, path: impl AsRef<Path>) -> Result<Pkg, ScanDirectoryError> {
        fn process_path(path: &Path) -> Result<Option<Module>, ScanDirectoryError> {
            let path_with_ext_wesl = path.with_extension("wesl");
            let path_with_ext_wgsl = path.with_extension("wgsl");
            let path_without_ext = path.with_extension("");
            let path_filename = path_without_ext
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();

            if !path_with_ext_wesl.is_file()
                && !path_with_ext_wgsl.is_file()
                && !path_without_ext.is_dir()
            {
                return Ok(None);
            }

            if RESERVED_MOD_NAMES.contains(&path_filename.as_str()) {
                return Err(ScanDirectoryError::ReservedModName(path_filename));
            }

            if !is_mod_ident(&path_filename) {
                // skip file or directory names that are not valid identifiers,
                // but WGSL reserved words not in RESERVED_MOD_NAMES are allowed.
                println!(
                    "cargo::warning=skipped file/dir: not a WGSL ident `{path_filename}` {path:?}"
                );
                return Ok(None);
            }

            // check for source file
            let source = if path_with_ext_wesl.is_file() {
                std::fs::read_to_string(&path_with_ext_wesl)?
            } else if path_with_ext_wgsl.is_file() {
                std::fs::read_to_string(&path_with_ext_wgsl)?
            } else {
                String::new()
            };

            // check for submodules
            let mut submodules = Vec::new();
            if path_without_ext.is_dir() {
                // use hashset to avoid duplicate entries
                let mut unique_submodules = HashSet::new();
                for entry in std::fs::read_dir(&path_without_ext)? {
                    let Ok(entry) = entry else { continue };
                    let submodule_path = entry.path().with_extension("");
                    unique_submodules.insert(submodule_path);
                }
                for entry in unique_submodules {
                    // errors in the top module should be returned
                    // other errors should only be logged
                    match process_path(&entry) {
                        Ok(Some(module)) => submodules.push(module),
                        Ok(None) => {}
                        Err(err) => {
                            println!("cargo::error=error processing submodule {entry:?}: {err}")
                        }
                    }
                }
            };

            // check for empty module
            if source.is_empty() && submodules.is_empty() {
                return Ok(None);
            }

            let module = Module {
                name: path_filename,
                source,
                submodules,
            };

            Ok(Some(module))
        }

        let root_path = path.as_ref().to_path_buf();
        let potential_module = process_path(&root_path)?;
        let Some(mut module) = potential_module else {
            return Err(ScanDirectoryError::RootNotFound(root_path));
        };
        // top level module should be named by package builder and not file path
        module.name = self.name;

        Ok(Pkg {
            crate_name: cargo_crate_name(),
            root: module,
            dependencies: self.dependencies,
        })
    }

    /// Reads a wesl.toml file and builds a package from its configuration.
    ///
    /// The wesl.toml file should be located in the given directory and contain:
    /// - `edition` (required), `root`, `include`, `exclude` as top-level keys
    /// - `[dependencies]` section (optional)
    ///
    /// # Example
    ///
    /// ```ignore
    /// wesl::PkgBuilder::new("my_package")
    ///     .scan_toml(".")  // looks for ./wesl.toml
    ///     .expect("failed to scan WESL files")
    ///     .build_artifact()
    ///     .expect("failed to build artifact");
    /// ```
    pub fn scan_toml(self, dir: impl AsRef<Path>) -> Result<Pkg, ScanTomlError> {
        let dir = dir.as_ref();
        let toml_path = dir.join("wesl.toml");

        if !toml_path.exists() {
            return Err(ScanTomlError::TomlNotFound(toml_path));
        }

        let config = WeslToml::from_file(&toml_path)?;
        let result = wesl_toml::scan_from_config(&self.name, dir, &config)?;

        for warning in &result.warnings {
            println!("cargo::warning={warning}");
        }

        Ok(Pkg {
            crate_name: cargo_crate_name(),
            root: result.module,
            dependencies: self.dependencies,
        })
    }
}

impl Module {
    fn codegen(&self) -> proc_macro2::TokenStream {
        let mod_ident = format_ident!("r#{}", self.name);
        let name = &self.name;
        let source = &self.source;

        let submodules = self.submodules.iter().map(|submod| {
            let name = &submod.name;
            let ident = format_ident!("r#{}", name);
            quote! { &#ident::MODULE }
        });

        let submods = self.submodules.iter().map(|submod| submod.codegen());

        quote! {
            pub mod #mod_ident {
                use super::CodegenModule;
                pub const MODULE: CodegenModule = CodegenModule {
                    name: #name,
                    source: #source,
                    submodules: &[#(#submodules),*]
                };

                #(#submods)*
            }
        }
    }

    fn validate(&self, parent_path: ModulePath) -> Result<(), Error> {
        let mut path = parent_path.clone();
        path.push(&self.name);

        eprintln!("INFO: validate {path}");

        let to_diagnostic = |e: Error| {
            Diagnostic::from(e)
                .with_module_path(path.clone(), None)
                .with_source(self.source.clone())
        };
        let mut wesl: TranslationUnit = self
            .source
            .parse()
            .map_err(|e: wgsl_parse::Error| to_diagnostic(e.into()))?;
        retarget_idents(&mut wesl);
        validate_wesl(&wesl).map_err(|e| to_diagnostic(e.into()))?;
        for module in &self.submodules {
            module.validate(path.clone())?;
        }
        Ok(())
    }
}

impl Pkg {
    /// Generate the rust code that holds the packaged wesl files.
    /// You probably want to use [`Self::build_artifact`] instead.
    pub fn codegen(&self) -> String {
        let deps = self.dependencies.iter().map(|dep| {
            let crate_name = format_ident!("r#{}", dep.crate_name);
            let mod_name = format_ident!("r#{}", dep.root.name);
            quote! { &#crate_name::#mod_name::PACKAGE }
        });

        let crate_name = &self.crate_name;
        let root_name = &self.root.name;
        let root_source = &self.root.source;

        let submodules = self.root.submodules.iter().map(|submod| {
            let name = &submod.name;
            let ident = format_ident!("r#{}", name);
            quote! { &#ident::MODULE }
        });

        let submods = self.root.submodules.iter().map(|submod| submod.codegen());

        let tokens = quote! {
            pub const PACKAGE: CodegenPkg = CodegenPkg {
                crate_name: #crate_name,
                root: &MODULE,
                dependencies: &[#(#deps),*],
            };

            pub const MODULE: CodegenModule = CodegenModule {
                name: #root_name,
                source: #root_source,
                submodules: &[#(#submodules),*]
            };

            #(#submods)*
        };

        tokens.to_string()
    }

    /// Run validation checks on each of the scanned files.
    pub fn validate(self) -> Result<Self, Error> {
        self.root.validate(ModulePath::new_root())?;
        Ok(self)
    }

    /// Generate the build artifact that can then be exposed by the [`super::wesl_pkg`] macro.
    ///
    /// This function must be called from a `build.rs` file. Refer to the crate documentation
    /// for more details.
    ///
    /// # Panics
    /// Panics if the OUT_DIR environment variable is not set. This should not happen if
    /// ran from a `build.rs` file.
    pub fn build_artifact(&self) -> std::io::Result<()> {
        let code = self.codegen();
        let out_dir = std::path::Path::new(
            &std::env::var_os("OUT_DIR").expect("OUT_DIR environment variable is not defined"),
        )
        .join(format!("{}.rs", self.root.name));
        std::fs::write(&out_dir, code)?;
        Ok(())
    }
}
