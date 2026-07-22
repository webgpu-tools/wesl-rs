use std::{collections::HashSet, path::Path};

use wgsl_parse::{
    SyntaxNode,
    syntax::{Ident, ModulePath, PathOrigin, TranslationUnit},
};

use crate::{
    SyntaxUtil,
    error::{Diagnostic, Error, ImportError, ResolveError},
    mangler::{self, Mangler},
    package::StaticPackage,
    pass::{self, CompilerDriver, Features, Module, UsedItems},
    resolver::{AsyncResolver, Constants, Resolver, StandardResolver},
    sourcemap::{BasicSourceMap, SourceMapper},
};

/// Compilation options. Used in [`compile`] and [`Wesl::set_options`].
#[derive(Clone, Debug)]
pub struct CompileOptions {
    /// Toggle [WESL Imports](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/Imports.md).
    ///
    /// If disabled:
    /// * The compiler will silently remove the import statements and inline paths.
    /// * Validation will not trigger an error if referencing an imported item.
    pub imports: bool,
    /// Toggle [WESL Conditional Translation](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/ConditionalTranslation.md).
    ///
    /// See `features` to enable/disable each feature flag.
    pub condcomp: bool,
    /// Toggle generics. Generics are super experimental, don't expect anything from it.
    ///
    /// Requires the `generics` crate feature flag.
    pub generics: bool,
    /// Enable stripping (aka. Dead Code Elimination).
    ///
    /// By default, all declarations reachable by entrypoint functions, const_asserts and
    /// pipeline-overridable constants are kept. See [`Self::keep`] and
    /// [`Self::keep_root`] to control what gets stripped.
    ///
    /// Stripping can have side-effects: modules are loaded only if statically accessed,
    /// and `const_assert` statements are not always preserved.
    /// Refer to the WESL docs to learn more.
    pub strip: bool,
    /// Enable lowering/polyfills. This transforms the output code in various ways.
    ///
    /// See [`lower`].
    pub lower: bool,
    /// Enable validation of individual WESL modules and the final output.
    /// This will catch *some* errors, not all.
    /// See [`validate_wesl`] and [`validate_wgsl`].
    ///
    /// Requires the `eval` crate feature flag.
    pub validate: bool,
    /// Enable sourcemapping, which provides better error diagnostics.
    pub sourcemap: bool,
    /// Declaration name mangling scheme.
    pub mangler: ManglerKind,
    /// Enable mangling of declarations in the root module.
    ///
    /// By default, WESL does not mangle root module declarations.
    pub mangle_root: bool,
    /// If `Some`, specify a list of root module declarations to keep. If `None`, only the
    /// entrypoint functions (and their dependencies) are kept.
    ///
    /// This option has no effect if [`Self::keep_root`] is enabled or  [`Self::strip`] is
    /// disabled.
    pub keep: Option<Vec<String>>,
    /// If `true`, all root module declarations are preserved when stripping is enabled.
    ///
    /// This option takes precedence over [`Self::keep`], and has no effect if
    /// [`Self::strip`] is disabled.
    pub keep_root: bool,
    /// [WESL Conditional Translation](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/ConditionalTranslation.md)
    /// Conditional translation feature flags.
    ///
    /// Conditional translation can be incremental. If not all feature flags are handled,
    /// the output will contain unevaluated `@if` attributes and will therefore *not* be
    /// valid WGSL.
    ///
    /// This option has no effect if [`Self::condcomp`] is disabled.
    pub features: Features,
    /// Literal constants in the `constants` virtual module.
    ///
    /// See [`Constants`].
    pub constants: Constants,
    /// Importable packages dependencies.
    pub dependencies: Vec<&'static StaticPackage>,
}

/// Declaration name mangling scheme. Used in [`Wesl::set_mangler`].
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManglerKind {
    /// Escaped path mangler.
    /// `foo_bar::item -> _1foo_bar_item`
    #[default]
    Escape,
    /// Hash mangler.
    /// `foo::bar::item -> item_1985638328947`
    Hash,
    /// Make valid identifiers with unicode "confusables" characters.
    /// `foo::bar<baz, moo> -> foo::barᐸbazˏmooᐳ`
    Unicode,
    /// Disable mangling. (warning: will break shaders if case of name conflicts!)
    None,
}

impl From<ManglerKind> for Box<dyn Mangler> {
    fn from(kind: ManglerKind) -> Self {
        match kind {
            ManglerKind::Escape => Box::new(mangler::EscapeMangler),
            ManglerKind::Hash => Box::new(mangler::HashMangler),
            ManglerKind::Unicode => Box::new(mangler::UnicodeMangler),
            ManglerKind::None => Box::new(mangler::NoMangler),
        }
    }
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            imports: true,
            condcomp: true,
            generics: false,
            strip: true,
            lower: false,
            validate: true,
            sourcemap: true,
            mangler: Default::default(),
            mangle_root: false,
            keep: Default::default(),
            keep_root: false,
            features: Default::default(),
            constants: Default::default(),
            dependencies: Default::default(),
        }
    }
}

/// The WESL compiler.
///
/// # Basic Usage
///
/// ```rust
/// # use wesl::{Compiler, resolver::VirtualResolver};
/// #
/// let compiler = Compiler::default();
/// #
/// # // just adding a virtual file here so the doctest runs without a filesystem
/// # let mut resolver = VirtualResolver::new();
/// # let shader_string = "fn my_fn() {\n\n}\n";
/// # resolver.add_module("package::path::to::shader".parse().unwrap(), shader_string.into());
/// # let mut compiler = compiler.set_resolver(resolver);
/// # compiler.options.keep_root = true; // prevent dead code elimination
/// #
/// let wgsl_string = compiler
///     .compile("path/to/shader.wgsl")
///     .inspect_err(|e| eprintln!("{e}")) // pretty-print errors
///     .unwrap()
///     .to_string();
/// #
/// # assert!(wgsl_string == shader_string);
/// ```
#[derive(Clone, Debug)]
pub struct Compiler<R = ()> {
    pub options: CompileOptions,
    pub resolver: R,
}

impl Default for Compiler<()> {
    fn default() -> Self {
        Self {
            options: Default::default(),
            resolver: Default::default(),
        }
    }
}

impl<R1> Compiler<R1> {
    pub fn set_resolver<R2>(self, resolver: R2) -> Compiler<R2> {
        Compiler::<R2> {
            options: self.options,
            resolver,
        }
    }
}

impl<R> Compiler<R> {
    pub fn new_with_resolver(options: CompileOptions, resolver: R) -> Self {
        Self { options, resolver }
    }
}

impl Compiler<()> {
    pub fn new(options: CompileOptions) -> Self {
        let resolver = ();
        Self { options, resolver }
    }
}

impl Compiler<()> {
    /// Compile a WESL shader to WGSL.
    ///
    /// `path` defines where to look for shader files. It can be either:
    /// * The path to a `wesl.toml` file, or a directory containing a `wesl.toml` file.
    ///   => The compiler follows wesl-toml semantics, refer its spec.
    /// * The path to a `.wesl` file.
    ///   => This file is the root module. Submodules are in an adjacent directory with the same name (extension stripped).
    /// * The path to a directory.
    ///   => An optional `package.wesl` file in the directory serves as the root module. Other `.wesl` files and subdirectories are submodules.
    ///
    /// Note: `.wgsl` extensions are also supported.
    pub fn compile(&self, path: impl AsRef<Path>) -> Result<CompileResult, Error> {
        let path = path.as_ref();

        let toml_cfg = if let Some(filename) = path.file_name()
            && filename == "wesl.toml"
        {
            Some(crate::toml_cfg::WeslToml::from_file(path)?)
        } else {
            None
        };

        let base_path = if let Some(cfg) = &toml_cfg {
            path.parent().unwrap(/* SAFETY: cannot fail if `file_name` succeeds */).join(&cfg.package.root)
        } else {
            path.to_path_buf()
        };

        let module_path = if base_path.is_file() {
            ModulePath::new_root()
        } else {
            // TODO: is this correct?
            ModulePath::new(PathOrigin::Absolute, vec!["package".to_string()])
        };

        self.compile_module(base_path, &module_path)
    }

    /// Compile a WESL shader to WGSL.
    pub fn compile_module(
        &self,
        base_path: impl AsRef<Path>,
        module_path: &ModulePath,
    ) -> Result<CompileResult, Error> {
        let mut resolver = StandardResolver::new(base_path.as_ref().with_extension(""));
        let mangler = Box::<dyn Mangler>::from(self.options.mangler);

        for (name, value) in self.options.constants.iter() {
            resolver.add_constant(name.clone(), value.clone());
        }

        for package in self.options.dependencies.iter() {
            resolver.add_package(package);
        }

        let sourcemapper = SourceMapper::new(module_path.clone(), &resolver, &mangler);

        let mut pass =
            CompilationPass::new(&module_path, &self.options, &sourcemapper, &sourcemapper);

        let res = CompilerDriver::compile(&mut pass);
        let sourcemap = sourcemapper.finish();
        let res = res.map_err(|e| Diagnostic::from(e).with_sourcemap(&sourcemap))?;

        Ok(CompileResult {
            syntax: res.syntax,
            sourcemap: Some(sourcemap),
            used_items: res.used_items,
        })
    }
}

impl<R: Resolver> Compiler<R> {
    /// Compile a WESL shader to WGSL.
    ///
    /// `path` defines where to look for shader files. It can be either:
    /// * The path to a `wesl.toml` file, or a directory containing a `wesl.toml` file.
    ///   => The compiler follows wesl-toml semantics, refer its spec.
    /// * The path to a `.wesl` file.
    ///   => This file is the root module. Submodules are in an adjacent directory with the same name (extension stripped).
    /// * The path to a directory.
    ///   => An optional `package.wesl` file in the directory serves as the root module. Other `.wesl` files and subdirectories are submodules.
    ///
    /// Note: `.wgsl` extensions are also supported.
    ///
    /// # Warning
    ///
    /// This function works best with filesystem resolvers which implement [`Resolver::fs_path`].
    /// With `fs_path` implemented, the function makes the input file path relative to the package's root directory.
    /// Otherwise, it converts the file path to a module path straight away, which may panic (see [`ModulePath::from_path`]).
    pub fn compile(&self, path: impl AsRef<Path>) -> Result<CompileResult, Error> {
        let path = path.as_ref();

        // TODO: rework implementations of fs_path to be more fault tolerant?
        // or add a function root_dir?
        let relative_path = if let Some(root) = self.resolver.fs_path(&ModulePath::new_root()) {
            let abs_path = std::path::absolute(path).map_err(|e| ResolveError::Io(e))?;
            let abs_root = std::path::absolute(root).map_err(|e| ResolveError::Io(e))?;
            let abs_root = abs_root.parent().unwrap_or(Path::new(""));
            abs_path
                .strip_prefix(abs_root)
                .unwrap_or(&path)
                .to_path_buf()
        } else {
            Path::new(".").join(path)
        };

        let mut root_module_path = ModulePath::from_path(relative_path);
        // we force the origin to be absolute if it was relative.
        root_module_path.origin = PathOrigin::Absolute;

        self.compile_module(&root_module_path)
    }

    /// Compile a WESL shader to WGSL.
    pub fn compile_module(&self, root_module_path: &ModulePath) -> Result<CompileResult, Error> {
        let mangler = Box::<dyn Mangler>::from(self.options.mangler);

        let mut pass =
            CompilationPass::new(root_module_path, &self.options, &self.resolver, &mangler);
        let res = CompilerDriver::compile(&mut pass)?;

        Ok(CompileResult {
            syntax: res.syntax,
            sourcemap: None,
            used_items: res.used_items,
        })
    }
}

#[derive(Default, Clone)]
pub struct CompileResult {
    pub syntax: TranslationUnit,
    pub sourcemap: Option<BasicSourceMap>,
    pub used_items: UsedItems,
}

impl CompileResult {
    pub fn write_to_file(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        std::fs::write(path, self.to_string())
    }

    /// Emit `rerun-if-changed` instructions so the build script reruns only if the
    /// shader files are modified.
    pub fn emit_rerun_if_changed(&self) {
        let Some(sourcemap) = &self.sourcemap else {
            println!("cargo::warning=cannot emit rerun-if-changed directive without a sourcemap");
            return;
        };

        for (module_path, _) in self.used_items.iter() {
            if module_path.origin.is_package() {
                continue;
            }
            assert!(
                !module_path.origin.is_relative(),
                "the modules passed to emit_rerun_if_changed must be absolute"
            );
            if let Some(source) = sourcemap.file(module_path)
                && let Some(fs_path) = &source.path
            {
                // Path::display is safe here because of the ModulePath naming restrictions
                println!("cargo::rerun-if-changed={}", fs_path.display());

                // If it's a fallback path, we need to react to the higher priority path as well
                if fs_path.extension().unwrap() == "wgsl" {
                    let fs_path = fs_path.with_extension("wesl");
                    println!("cargo::rerun-if-changed={}", fs_path.display());
                }
            }
        }
    }

    /// Write the result in rust's `OUT_DIR`.
    ///
    /// This function is meant to be used in a `build.rs` workflow. The output WGSL will
    /// be accessed with the [`include_wesl`] macro. See the crate documentation for a
    /// usage example.
    ///
    /// # Panics
    ///
    /// Panics when the output file cannot be written.
    pub fn write_artifact(&self, artifact_name: &str) {
        let dirname = std::env::var("OUT_DIR").unwrap();
        let out_name = Path::new(artifact_name);
        if out_name.iter().count() != 1 || out_name.extension().is_some() {
            eprintln!("`out_name` cannot contain path separators or file extension");
            panic!()
        }
        let mut output = Path::new(&dirname).join(out_name);
        output.set_extension("wgsl");
        self.write_to_file(output)
            .expect("failed to write output shader");
    }
}

impl std::fmt::Display for CompileResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.syntax.fmt(f)
    }
}

pub struct CompilationPass<'a> {
    root_path: &'a ModulePath,
    options: &'a CompileOptions,
    resolver: &'a dyn Resolver,
    mangler: &'a dyn Mangler,
}

// pub struct AsyncCompilationPass<'a> {
//     async_resolver: &'a dyn AsyncResolver,
// }

impl<'a> CompilationPass<'a> {
    fn new(
        root_path: &'a ModulePath,
        options: &'a CompileOptions,
        resolver: &'a dyn Resolver,
        mangler: &'a dyn Mangler,
    ) -> Self {
        Self {
            root_path,
            options,
            resolver,
            mangler,
        }
    }
}

pub async fn load_module_async(
    path: &ModulePath,
    resolver: &impl AsyncResolver,
) -> Result<TranslationUnit, Error> {
    let source = resolver.resolve_source_async(path).await?;

    let mut module: TranslationUnit = source.parse().map_err(|e| {
        Diagnostic::from(e)
            .with_module_path(path.clone(), resolver.display_name(path))
            .with_source(source.to_string())
    })?;

    pass::retarget_idents(&mut module);

    Ok(module)
}

impl CompilerDriver for CompilationPass<'_> {
    fn root_path(&self) -> &ModulePath {
        &self.root_path
    }

    fn root_entry_points(&self, root_module: &TranslationUnit) -> Result<HashSet<Ident>, Error> {
        // keep all declarations when strip is disabled or keep_root is enabled.
        if !self.options.strip || self.options.keep_root {
            Ok(root_module
                .global_declarations
                .iter()
                .filter_map(|decl| decl.ident())
                .collect())
        }
        // user provided an explicit list of entry points to start from.
        else if let Some(keep) = &self.options.keep {
            keep.iter()
                .map(|name| {
                    root_module.decl_ident(name).ok_or_else(|| {
                        ImportError::MissingDecl(self.root_path.clone(), name.to_string()).into()
                    })
                })
                .collect::<Result<HashSet<Ident>, Error>>()
        }
        // otherwise, we keep the WGSL entry points. this is the default.
        else {
            Ok(root_module.entry_points().collect())
        }
    }

    fn load_module(&mut self, path: &ModulePath) -> Result<TranslationUnit, Error> {
        let mut module = pass::load_module(path, &self.resolver)?;

        if self.options.condcomp {
            pass::condcomp(&mut module, &self.options.features)?;
        }

        pass::retarget_idents(&mut module);

        if self.options.validate {
            pass::validate_wesl(&module)?;
        }

        Ok(module)
    }

    fn link(
        &self,
        modules: &mut Vec<Module>,
        used_items: &UsedItems,
    ) -> Result<TranslationUnit, Error> {
        pass::retarget_modules(modules, used_items);

        for module in modules.iter_mut() {
            if !self.options.mangle_root && module.path == *self.root_path {
                continue;
            }
            pass::mangle(&mut module.syntax, &module.path, &self.mangler);
        }

        let mut module = pass::link(modules, self.options.strip.then_some(used_items));

        if self.options.lower {
            pass::lower(&mut module)?;
        }

        if self.options.validate {
            pass::validate_wgsl(&module)?;
        }

        Ok(module)
    }
}

// impl pipeline::AsyncCompilerDriver for CompilationPass<'_> {
//     async fn load_module_async(&mut self, path: &ModulePath) -> Result<TranslationUnit, Error> {
//         let mut module = pipeline::load_module_async(path, &self.resolver).await?;

//         if self.options.condcomp {
//             pass::condcomp(&mut module, &self.options.features)?;
//         }

//         if self.options.validate {
//             pass::validate_wesl(&module)?;
//         }

//         Ok(module)
//     }
// }

#[test]
fn test_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Compiler<()>>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures_dir() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/compile"
        ))
    }

    // basically an expansion of the `wesl_pkg` macro.
    mod package_random {
        use crate::package::{StaticPackage, StaticPackageModule};
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/compile/package_random.rs"
        ));
    }

    #[test]
    fn compile_simple() {
        let test_path = fixtures_dir().join("simple/shaders/main.wgsl");

        let mut compiler = Compiler::default();

        // "lower" behaves differently when "eval" is enabled.
        let lower_name = if cfg!(feature = "eval") {
            "simple_lower_eval"
        } else {
            "simple_lower"
        };

        compiler.options.lower = false;
        compiler.options.strip = false;
        let mut result = compiler.compile(&test_path).unwrap();
        result.syntax.sort_decls(); // normalize for comparison
        insta::assert_snapshot!("simple", result.syntax.to_string());

        compiler.options.lower = false;
        compiler.options.strip = true;
        let mut result = compiler.compile(&test_path).unwrap();
        result.syntax.sort_decls(); // normalize for comparison
        insta::assert_snapshot!("simple_strip", result.syntax.to_string());

        compiler.options.lower = true;
        compiler.options.strip = false;
        let mut result = compiler.compile(&test_path).unwrap();
        result.syntax.sort_decls(); // normalize for comparison
        insta::assert_snapshot!(lower_name, result.syntax.to_string());
    }

    #[test]
    fn compile_toml() {
        let test_path = fixtures_dir().join("complex/wesl.toml");

        let features = Features::new();

        let mut constants = Constants::new();
        constants.add_constant("PI", 3.1415);
        constants.add_constant("TRUE", true);

        let mut compiler = Compiler::new(CompileOptions {
            features,
            constants,
            dependencies: vec![&package_random::PACKAGE],
            ..Default::default()
        });

        compiler.options.features.add_feature("feat1", true);
        compiler.options.features.add_feature("feat2", false);
        let mut result = compiler.compile(&test_path).unwrap();
        result.syntax.sort_decls(); // normalize for comparison
        insta::assert_snapshot!("toml_feat1", result.syntax.to_string());

        compiler.options.features.add_feature("feat1", false);
        compiler.options.features.add_feature("feat2", true);
        let mut result = compiler.compile(&test_path).unwrap();
        result.syntax.sort_decls(); // normalize for comparison
        insta::assert_snapshot!("toml_feat2", result.syntax.to_string());
    }
}
