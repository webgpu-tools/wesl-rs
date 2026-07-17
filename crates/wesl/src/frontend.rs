use std::{collections::HashSet, path::Path};

use wgsl_parse::{
    SyntaxNode,
    syntax::{Ident, ModulePath, TranslationUnit},
};

use crate::{
    Features, SyntaxUtil,
    error::{Diagnostic, Error, ImportError},
    mangler::Mangler,
    pass::{self, UsedItems},
    pipeline,
    resolver::{AsyncResolver, Constants, Resolver, StaticPackage},
    sourcemap::BasicSourceMap,
    wesl_toml::WeslToml,
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

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            imports: true,
            condcomp: true,
            generics: false,
            strip: true,
            lower: false,
            validate: true,
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
/// # use wesl::{Wesl, VirtualResolver};
/// #
/// let compiler = Wesl::new("path/to/dir/containing/shaders");
/// #
/// # // just adding a virtual file here so the doctest runs without a filesystem
/// # let mut resolver = VirtualResolver::new();
/// # resolver.add_module("package::main".parse().unwrap(), "fn my_fn() {}".into());
/// # let compiler = compiler.set_custom_resolver(resolver);
/// #
/// let wgsl_string = compiler
///     .compile(&"package::main".parse().unwrap())
///     .unwrap()
///     .to_string();
/// ```
#[derive(Default, Clone, Debug)]
pub struct Compiler {
    options: CompileOptions,
}

impl Compiler {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    pub fn compile(path: impl AsRef<Path>) -> Result<CompileResult, Error> {
        let path = path.as_ref();

        if let Some(filename) = path.file_name()
            && filename == "wesl.toml"
        {
            let toml_cfg = WeslToml::from_file(path)?;
            let base_path = path.parent().unwrap(/* SAFETY: cannot fail if `file_name` succeeds */).join(&toml_cfg.package.root);
        }

        // let resolver = Box::new(StandardResolver::new(base));
        // let mangler = Box::new(EscapeMangler);
        // let sourcemapper = SourceMapper::new(root.clone(), resolver, mangler);

        Ok(todo!())
    }

    // pub fn compile_with(path: impl AsRef<Path>) {
    //     let resolver = Box::new(StandardResolver::new(base));
    //     let mangler = Box::new(EscapeMangler);
    //     let sourcemapper = SourceMapper::new(root.clone(), resolver, mangler);
    // }
}

pub struct CompileResult {
    pub syntax: TranslationUnit,
    pub sourcemap: BasicSourceMap,
    pub modules: Vec<ModulePath>,
}

pub struct CompilationPass<'a> {
    root_path: &'a ModulePath,
    options: &'a CompileOptions,
    resolver: &'a dyn Resolver,
    mangler: &'a dyn Mangler,
}

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

impl pipeline::CompilerDriver for CompilationPass<'_> {
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
        let mut module = pipeline::load_module(path, &self.resolver)?;

        if self.options.condcomp {
            pass::condcomp(&mut module, &self.options.features)?;
        }

        if self.options.validate {
            pass::validate_wesl(&module)?;
        }

        Ok(module)
    }

    fn link(
        &self,
        mut modules: Vec<pipeline::Module>,
        used_items: &UsedItems,
    ) -> Result<TranslationUnit, Error> {
        for module in &mut modules {
            pass::mangle(&mut module.syntax, &module.path, &self.mangler);
        }

        let mut module = pass::link(&modules, self.options.strip.then_some(used_items));

        if self.options.lower {
            pass::lower(&mut module)?;
        }

        if self.options.validate {
            pass::validate_wgsl(&module)?;
        }

        Ok(module)
    }
}
