//! WESL compiler frontend.
//!
//! Types and functions in this module are small wrappers that make it easy to use WESL in typical scenarios, "batteries included".

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

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
    resolver::{Constants, Resolver, StandardResolver},
    sourcemap::{BasicSourceMap, SourceMapper},
};

/// Compilation options used by [`Compiler`].
#[derive(Clone, Debug, PartialEq)]
pub struct CompileOptions {
    /// Toggle [WESL Imports](https://github.com/webgpu-tools/wesl-spec/blob/main/Imports.md).
    ///
    /// If disabled:
    ///
    /// * The compiler will silently remove all import statements and inline paths.
    /// * Validation will not trigger an error if referencing an imported item.
    pub imports: bool,
    /// Toggle [WESL Conditional Translation](https://github.com/webgpu-tools/wesl-spec/blob/main/ConditionalTranslation.md).
    ///
    /// See [`Self::features`] to enable/disable each feature flag.
    pub condcomp: bool,
    /// Toggle generics. Generics are super experimental, don't expect anything from it.
    ///
    /// Requires the `generics` crate feature flag.
    pub generics: bool,
    /// Enable stripping (aka. Dead Code Elimination).
    ///
    /// By default, all declarations reachable by entrypoint functions, const_asserts and
    /// pipeline-overridable constants in the main module are kept.
    /// See [`Self::keep`] and [`Self::keep_main`] to control what gets stripped.
    ///
    /// Stripping can have side-effects: modules are loaded only if statically accessed,
    /// and `const_assert` statements are not always preserved.
    /// Refer to the WESL docs to learn more.
    pub strip: bool,
    /// Enable lowering/polyfills. This transforms the output code in various ways.
    ///
    /// See [`pass::lower`] for the list of transforms.
    pub lower: bool,
    /// Enable validation of individual WESL modules and of the final output.
    ///
    /// This will catch *some* errors, not all.
    /// See [`pass::validate_wesl`] and [`pass::validate_wgsl`] for the list of validations.
    ///
    /// Requires the `eval` crate feature flag.
    pub validate: bool,
    /// Enable sourcemapping, which provides better error diagnostics.
    pub sourcemap: bool,
    /// Declaration name mangling scheme.
    pub mangler: ManglerKind,
    /// Enable mangling of declarations in the main module.
    ///
    /// By default, WESL does not mangle main module declarations.
    pub mangle_main: bool,
    /// If `Some`, specify a list of main module declarations to keep.
    /// If `None`, only the entrypoint functions (and their dependencies) are kept.
    ///
    /// This option has no effect if [`Self::keep_main`] is enabled or  [`Self::strip`] is
    /// disabled.
    pub keep: Option<Vec<String>>,
    /// If `true`, all main module declarations are preserved when stripping is enabled.
    ///
    /// This option takes precedence over [`Self::keep`], and has no effect if
    /// [`Self::strip`] is disabled.
    pub keep_main: bool,
    /// Conditional Translation feature flags.
    ///
    /// This option has no effect if [`Self::condcomp`] is disabled.
    ///
    /// See [`Features`].
    pub features: Features,
    /// Literal constants in the `constants` virtual module.
    ///
    /// See [`Constants`].
    pub constants: Constants,
    /// Importable packages dependencies.
    pub dependencies: Vec<&'static StaticPackage>,
}

/// Declaration name mangling scheme. Used in [`CompileOptions::mangler`].
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
            mangle_main: false,
            keep: Default::default(),
            keep_main: false,
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
/// # let mut compiler = compiler.with_resolver(resolver);
/// # compiler.options.keep_main = true; // prevent dead code elimination
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
    /// Set the compilation [`crate::Resolver`] or [`crate::AsyncResolver`].
    pub fn with_resolver<R2>(self, resolver: R2) -> Compiler<R2> {
        Compiler::<R2> {
            options: self.options,
            resolver,
        }
    }
}

impl<R> Compiler<R> {
    /// Shorthand for `Compiler::new(options).with_resolver(resolver)`.
    pub fn new_with_resolver(options: CompileOptions, resolver: R) -> Self {
        Self { options, resolver }
    }
}

impl Compiler<()> {
    /// Create a new compiler.
    ///
    /// By default, the compiler will use a [`StandardResolver`] when compiling.
    pub fn new(options: CompileOptions) -> Self {
        let resolver = ();
        Self { options, resolver }
    }
}

/// Standalone version of [`Compiler::compile_module`].
pub fn compile(
    main_path: &ModulePath,
    options: &CompileOptions,
    resolver: &impl Resolver,
) -> Result<CompileResult, Error> {
    let mangler = Box::<dyn Mangler>::from(options.mangler);

    if options.sourcemap {
        let sourcemapper = SourceMapper::new(main_path.clone(), &resolver, &mangler);
        let mut pass = CompilationPass::new(main_path, options, &sourcemapper, &sourcemapper);
        let res = CompilerDriver::compile(&mut pass);
        let sourcemap = sourcemapper.finish();
        let res = res.map_err(|e| Diagnostic::from(e).with_sourcemap(&sourcemap))?;

        Ok(CompileResult {
            syntax: res.syntax,
            sourcemap: Some(sourcemap),
            used_items: res.used_items,
        })
    } else {
        let mut pass = CompilationPass::new(main_path, options, &resolver, &mangler);
        let res = CompilerDriver::compile(&mut pass)?;
        Ok(CompileResult {
            syntax: res.syntax,
            sourcemap: None,
            used_items: res.used_items,
        })
    }
}

/// Async version of [`compile`].
pub async fn compile_async(
    main_path: &ModulePath,
    options: &CompileOptions,
    resolver: &impl Resolver,
) -> Result<CompileResult, Error> {
    let mangler = Box::<dyn Mangler>::from(options.mangler);

    if options.sourcemap {
        let sourcemapper = SourceMapper::new(main_path.clone(), &resolver, &mangler);
        let mut pass = CompilationPass::new(main_path, options, &sourcemapper, &sourcemapper);
        let res = CompilerDriver::compile(&mut pass);
        let sourcemap = sourcemapper.finish();
        let res = res.map_err(|e| Diagnostic::from(e).with_sourcemap(&sourcemap))?;

        Ok(CompileResult {
            syntax: res.syntax,
            sourcemap: Some(sourcemap),
            used_items: res.used_items,
        })
    } else {
        let mut pass = CompilationPass::new(main_path, options, &resolver, &mangler);
        let res = CompilerDriver::compile_async(&mut pass).await?;
        Ok(CompileResult {
            syntax: res.syntax,
            sourcemap: None,
            used_items: res.used_items,
        })
    }
}

/// Get the module path for a filesystem path relative to the package root directory,
/// with a fallback to the current working directory if the resolver doesn't support
/// filesystem mappings.
///
/// # Panics
///
/// [`ModulePath::from_path`] can panic.
fn main_module_path(path: &Path, resolver: &impl Resolver) -> Result<ModulePath, Error> {
    let mut main_path = resolver.module_path(path).or_else(|e| {
        if matches!(e, ResolveError::FilesystemNotSupported) {
            Ok(ModulePath::from_path(Path::new(".").join(path)))
        } else {
            Err(e)
        }
    })?;

    // we force the origin to be absolute if it was relative.
    main_path.origin = PathOrigin::Absolute;
    Ok(main_path)
}

impl Compiler<()> {
    // TODO: implement and validate wesl-toml semantics.
    /// Compile a WESL shader to WGSL.
    ///
    /// `path` defines where to look for shader files.
    /// It can point to a `wesl.toml` file, a directory or a shader file.
    ///
    /// The main module (which exposes entry points, bindings and overrides) is determined from `path`:
    /// It is the package root module if `path` points to a toml file or a directory,
    /// otherwise it is the file that `path` points to.
    /// See [`Self::compile_module`] to compile a different main module.
    ///
    /// | Path         | Package root directory      | Main module                |
    /// | ------------ | --------------------------- | -------------------------- |
    /// | `wesl.toml`  | `root` field in `wesl.toml` | `package.wesl` in root dir |
    /// | directory    | the directory               | `package.wesl` in root dir |
    /// | `.wesl` file | the parent directory        | the file specified         |
    ///
    /// Note: `.wgsl` extensions are also supported, but `.wesl` takes priority.
    pub fn compile(&self, path: impl AsRef<Path>) -> Result<CompileResult, Error> {
        let (pkg_root_dir, main_path) = self.root_and_main(path.as_ref())?;
        self.compile_module(pkg_root_dir, &main_path)
    }

    /// Variant of [`Self::compile`] with a custom main module path.
    pub fn compile_module(
        &self,
        pkg_root_dir: impl AsRef<Path>,
        main_path: &ModulePath,
    ) -> Result<CompileResult, Error> {
        let resolver = self.create_resolver(pkg_root_dir.as_ref());
        compile(main_path, &self.options, &resolver)
    }

    /// Async version of [`Self::compile`].
    pub async fn compile_async(&self, path: &Path) -> Result<CompileResult, Error> {
        let (pkg_root_dir, main_path) = self.root_and_main(path.as_ref())?;
        self.compile_module_async(&pkg_root_dir, &main_path).await
    }

    /// Async version of [`Self::compile_module`].
    pub async fn compile_module_async(
        &self,
        pkg_root_dir: impl AsRef<Path>,
        main_path: &ModulePath,
    ) -> Result<CompileResult, Error> {
        let resolver = self.create_resolver(pkg_root_dir.as_ref());
        compile_async(main_path, &self.options, &resolver).await
    }

    fn create_resolver(&self, pkg_root_dir: &Path) -> StandardResolver {
        let mut resolver = StandardResolver::new(pkg_root_dir);

        for (name, value) in self.options.constants.iter() {
            resolver.add_constant(name.clone(), *value);
        }

        for package in self.options.dependencies.iter() {
            resolver.add_package(package);
        }

        resolver
    }

    // TODO: implement and validate wesl-toml semantics.
    fn root_and_main(&self, path: &Path) -> Result<(PathBuf, ModulePath), Error> {
        let toml_cfg = if let Some(filename) = path.file_name()
            && filename == "wesl.toml"
        {
            Some(crate::toml_cfg::WeslToml::from_file(path)?)
        } else {
            None
        };

        let pkg_root_dir = if let Some(cfg) = &toml_cfg {
            path.parent().unwrap(/* SAFETY: cannot fail if `file_name` succeeds */).join(&cfg.package.root)
        } else {
            path.to_path_buf()
        };

        let main_path = if pkg_root_dir.is_file() {
            ModulePath::new_root()
        } else {
            ModulePath::new(PathOrigin::Absolute, vec!["package".to_string()])
        };

        Ok((pkg_root_dir, main_path))
    }
}

impl<R: Resolver> Compiler<R> {
    /// Compile a WESL shader to WGSL.
    ///
    /// The main module defaults to the package root module.
    /// See [`Self::compile_module`] to compile a different main module.
    pub fn compile_root(&self) -> Result<CompileResult, Error> {
        compile(&ModulePath::new_root(), &self.options, &self.resolver)
    }

    /// Variant of [`Self::compile`] with a custom main module path.
    pub fn compile_module(&self, main_path: &ModulePath) -> Result<CompileResult, Error> {
        compile(main_path, &self.options, &self.resolver)
    }

    /// Variant of [`Self::compile_root`] with a custom main module path.
    ///
    /// `fs_main_path` defines the main module path according to the resolver's file system mapping implemented in [`Resolver::module_path`].
    ///
    /// # Warning
    ///
    /// This function works best with filesystem resolvers which implement [`Resolver::module_path`].
    /// If not, this function assumes that the package root directory is the current working directory.
    ///
    /// # Panics
    ///
    /// Can panic if [`ModulePath::from_path`] fails.
    // TODO: we don't want that panic.
    pub fn compile(&self, fs_main_path: impl AsRef<Path>) -> Result<CompileResult, Error> {
        let main_path = main_module_path(fs_main_path.as_ref(), &self.resolver)?;
        compile(&main_path, &self.options, &self.resolver)
    }

    /// Async version of [`Self::compile_root`].
    pub async fn compile_root_async(&self) -> Result<CompileResult, Error> {
        compile_async(&ModulePath::new_root(), &self.options, &self.resolver).await
    }

    /// Async version of [`Self::compile_module`].
    pub async fn compile_module_async(
        &self,
        main_path: &ModulePath,
    ) -> Result<CompileResult, Error> {
        compile_async(main_path, &self.options, &self.resolver).await
    }

    /// Async version of [`Self::compile`].
    pub async fn compile_async(
        &self,
        fs_main_path: &impl AsRef<Path>,
    ) -> Result<CompileResult, Error> {
        let main_path = main_module_path(fs_main_path.as_ref(), &self.resolver)?;
        compile_async(&main_path, &self.options, &self.resolver).await
    }
}

/// Result of [`Compiler::compile`].
///
/// This type contains the resulting WGSL syntax tree, the sourcemap (if enabled),
/// and the list of used modules/declarations.
///
/// It implements [`std::fmt::Display`], call `to_string()` to get the compiled WGSL.
#[derive(Default, Clone)]
pub struct CompileResult {
    /// The syntax tree of the resulting
    pub syntax: TranslationUnit,
    pub sourcemap: Option<BasicSourceMap>,
    pub used_items: UsedItems,
}

impl CompileResult {
    /// Write the compiled result to a file.
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
    /// be accessed with the [`crate::include_wesl`] macro. See the crate documentation for a
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

/// Ephemeral type that implements [`CompilerDriver`] for a single compilation pass in [`Compiler::compile`]
struct CompilationPass<'a> {
    main_path: &'a ModulePath,
    options: &'a CompileOptions,
    resolver: &'a dyn Resolver,
    mangler: &'a dyn Mangler,
}

impl<'a> CompilationPass<'a> {
    fn new(
        main_path: &'a ModulePath,
        options: &'a CompileOptions,
        resolver: &'a dyn Resolver,
        mangler: &'a dyn Mangler,
    ) -> Self {
        Self {
            main_path,
            options,
            resolver,
            mangler,
        }
    }
}

impl CompilerDriver for CompilationPass<'_> {
    fn main_path(&self) -> &ModulePath {
        self.main_path
    }

    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        self.resolver.canonical_path(path)
    }

    fn main_entry_points(&self, main_module: &TranslationUnit) -> Result<HashSet<Ident>, Error> {
        // keep all declarations when strip is disabled or keep_main is enabled.
        if !self.options.strip || self.options.keep_main {
            Ok(main_module
                .global_declarations
                .iter()
                .filter_map(|decl| decl.ident())
                .collect())
        }
        // user provided an explicit list of entry points to start from.
        else if let Some(keep) = &self.options.keep {
            keep.iter()
                .map(|name| {
                    main_module.decl_ident(name).ok_or_else(|| {
                        ImportError::MissingDecl(self.main_path.clone(), name.to_string()).into()
                    })
                })
                .collect::<Result<HashSet<Ident>, Error>>()
        }
        // otherwise, we keep the WGSL entry points. this is the default.
        else {
            Ok(main_module.entry_points().collect())
        }
    }

    fn module_usage_analysis(
        &self,
        module: &Module,
        already_used: &mut UsedItems,
        to_analyze: &mut UsedItems,
    ) -> Result<(), Error> {
        pass::module_usage_analysis(module, already_used, to_analyze);

        // when strip is disabled, all declarations in the module are included so they
        // must be usage-analyzed.
        if !self.options.strip {
            for decl in &module.syntax.global_declarations {
                if let Some(ident) = decl.ident() {
                    self.usage_analysis(module, &ident.name(), already_used, to_analyze)?;
                }
            }
        }

        Ok(())
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
        pass::retarget_modules(modules, used_items, &self.resolver);

        for module in modules.iter_mut() {
            if !self.options.mangle_main && module.path == *self.main_path {
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

#[test]
fn test_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Compiler<()>>();
}
