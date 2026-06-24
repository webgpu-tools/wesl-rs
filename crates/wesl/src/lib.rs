#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "eval")]
pub mod eval;
#[cfg(feature = "generics")]
mod generics;
#[cfg(feature = "package")]
mod package;
#[cfg(feature = "package")]
mod wesl_toml;

mod condcomp;
mod error;
mod idents;
mod import;
mod lower;
mod mangle;
mod resolve;
mod sourcemap;
mod strip;
mod syntax_util;
mod validate;
mod visit;

#[cfg(feature = "eval")]
pub use eval::{Eval, EvalError, Exec, Inputs, exec_entrypoint};

#[cfg(feature = "generics")]
pub use generics::GenericsError;

#[cfg(feature = "package")]
pub use package::{Module, Pkg, PkgBuilder, ScanDirectoryError};
#[cfg(feature = "package")]
pub use wesl_toml::{ScanTomlError, WeslToml};

pub use condcomp::{CondCompError, Feature, Features};
pub use error::{Diagnostic, Error};
pub use import::ImportError;
pub use lower::lower;
pub use mangle::{CacheMangler, EscapeMangler, HashMangler, Mangler, NoMangler, UnicodeMangler};
pub use resolve::{
    CodegenModule, CodegenPkg, FileResolver, NoResolver, PkgResolver, Preprocessor, ResolveError,
    Resolver, Router, StandardResolver, VirtualResolver, emit_rerun_if_changed,
};
pub use sourcemap::{BasicSourceMap, NoSourceMap, SourceMap, SourceMapper};
pub use syntax_util::SyntaxUtil;
pub use validate::{ValidateError, validate_wesl, validate_wgsl};

// re-exports
pub use wesl_macros::*;
pub use wgsl_parse::syntax;
pub use wgsl_parse::syntax::ModulePath;

#[cfg(feature = "eval")]
use std::collections::HashMap;
use std::{collections::HashSet, fmt::Display, path::Path};

use strip::strip_except;
use wgsl_parse::{
    SyntaxNode,
    syntax::{Ident, TranslationUnit},
};
use wgsl_types::inst::LiteralInstance;

/// Compilation options. Used in [`compile`] and [`Wesl::set_options`].
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Make the import resolution lazy (This is the default mandated by WESL).
    ///
    /// The "lazy" import algorithm will only read a submodule is one of its item is used
    /// by the entrypoints or `keep` declarations (recursively via static usage analysis).
    /// In contrast, the "eager" import algorithm will follow all import paths.
    ///
    /// "lazy" is automatically disabled when stripping is disabled.
    pub lazy: bool,
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
    /// features to enable/disable.
    ///
    /// Conditional translation can be incremental. If not all feature flags are handled,
    /// the output will contain unevaluated `@if` attributes and will therefore *not* be
    /// valid WGSL.
    ///
    /// This option has no effect if [`Self::condcomp`] is disabled.
    pub features: Features,
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
            lazy: true,
            mangle_root: false,
            keep: Default::default(),
            keep_root: false,
            features: Default::default(),
        }
    }
}

/// Mangling scheme. Used in [`Wesl::set_mangler`].
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

/// Include a WGSL file compiled with [`Wesl::build_artifact`] as a string.
///
/// The argument corresponds to the `artifact_name` passed to [`Wesl::build_artifact`].
///
/// This is a very simple convenience macro. See the crate documentation for a usage
/// example.
#[macro_export]
macro_rules! include_wesl {
    ($root:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/", $root, ".wgsl"))
    };
}

/// Include a generated package.
///
/// Generates a module containing the code generated by the packager.
///
/// See [`PkgBuilder`] for more information about building WESL packages.
///
/// ```ignore
/// // Load package in `OUT_DIR/my_package.rs`
/// wesl_pkg!(my_package);
///
/// // make module `pub`
/// wesl_pkg!(pub my_package);
///
/// // add module attributes
/// wesl_pkg!(#[allow(clippy::all)] pub my_package);
/// ```
#[macro_export]
macro_rules! wesl_pkg {
    ($(#[$attr:meta])* $vis:vis $pkg_name:ident) => {
        $crate::wesl_pkg!($(#[$attr])* $vis $pkg_name, concat!(stringify!($pkg_name), ".rs"));
    };
    ($(#[$attr:meta])* $vis:vis $pkg_name:ident, $source:expr) => {
        $(#[$attr])* $vis mod $pkg_name {
            #![allow(non_snake_case)]
            use $crate::{CodegenModule, CodegenPkg};
            include!(concat!(env!("OUT_DIR"), "/", $source));
        }
    };
}

/// The WESL compiler high-level API.
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
pub struct Wesl<R: Resolver> {
    options: CompileOptions,
    use_sourcemap: bool,
    resolver: R,
    mangler: Box<dyn Mangler + Send + Sync + 'static>,
}

impl Wesl<StandardResolver> {
    /// Get a WESL compiler with all *mandatory* and *optional* WESL extensions enabled,
    /// but not *experimental* and *non-standard* extensions.
    ///
    /// See also: [`Wesl::new_barebones`], [`Wesl::new_experimental`].
    ///
    /// # WESL Reference
    /// This WESL compiler is spec-compliant.
    ///
    /// Mandatory extensions: imports, conditional translation.
    /// Optional extrensions: stripping.
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            options: CompileOptions::default(),
            use_sourcemap: true,
            resolver: StandardResolver::new(base),
            mangler: Box::new(EscapeMangler),
        }
    }

    /// Get a WESL compiler with all functionalities enabled, including *experimental* and
    /// *non-standard* ones.
    ///
    /// See also: [`Wesl::new`] and [`Wesl::new_barebones`].
    ///
    /// # WESL Reference
    /// This WESL compiler is *not* spec-compliant because it enables all extensions
    /// including *experimental* and *non-standard* ones. See [`Wesl::new`].
    ///
    /// Experimental extensions: generics.
    /// Non-standard extensions: `@const`.
    pub fn new_experimental(base: impl AsRef<Path>) -> Self {
        Self {
            options: CompileOptions {
                generics: true,
                lower: true,
                ..Default::default()
            },
            use_sourcemap: true,
            resolver: StandardResolver::new(base),
            mangler: Box::new(EscapeMangler),
        }
    }

    /// Add a package dependency.
    ///
    /// Learn more about packages in [`PkgBuilder`].
    pub fn add_package(&mut self, pkg: &'static CodegenPkg) -> &mut Self {
        self.resolver.add_package(pkg);
        self
    }

    /// Add several package dependencies.
    ///
    /// Learn more about packages in [`PkgBuilder`].
    pub fn add_packages(
        &mut self,
        pkgs: impl IntoIterator<Item = &'static CodegenPkg>,
    ) -> &mut Self {
        for pkg in pkgs {
            self.resolver.add_package(pkg);
        }
        self
    }

    /// Add a const-declaration to the special `constants` module.
    ///
    /// See [`StandardResolver::add_constant`].
    pub fn add_constant(&mut self, name: impl ToString, value: LiteralInstance) -> &mut Self {
        self.resolver.add_constant(name, value);
        self
    }

    /// Add several const-declarations to the special `constants` module.
    ///
    /// See [`StandardResolver::add_constant`].
    pub fn add_constants(
        &mut self,
        constants: impl IntoIterator<Item = (impl ToString, LiteralInstance)>,
    ) -> &mut Self {
        for (name, value) in constants {
            self.resolver.add_constant(name, value);
        }
        self
    }
}

impl Wesl<NoResolver> {
    /// Get a WESL compiler with no extensions, no mangler and no resolver.
    ///
    /// You must set a [`Mangler`] and a [`Resolver`] manually to use this compiler,
    /// see [`Wesl::set_mangler`] and [`Wesl::set_custom_resolver`].
    ///
    /// # WESL Reference
    /// This WESL compiler is *not* spec-compliant because it does not enable *mandatory*
    /// WESL extensions. See [`Wesl::new`].
    pub fn new_barebones() -> Self {
        Self {
            options: CompileOptions {
                imports: false,
                condcomp: false,
                generics: false,
                strip: false,
                lower: false,
                validate: false,
                lazy: false,
                mangle_root: false,
                keep: None,
                keep_root: false,
                features: Default::default(),
            },
            use_sourcemap: false,
            resolver: NoResolver,
            mangler: Box::new(NoMangler),
        }
    }
}

impl<R: Resolver> Wesl<R> {
    /// Set all compilation options.
    pub fn set_options(&mut self, options: CompileOptions) -> &mut Self {
        self.options = options;
        self
    }

    /// Set the [`Mangler`].
    ///
    /// The default mangler is [`EscapeMangler`].
    ///
    /// # WESL Reference
    /// Custom manglers *must* conform to the constraints described in [`Mangler`].
    ///
    /// Spec: not yet available.
    pub fn set_mangler(&mut self, kind: ManglerKind) -> &mut Self {
        self.mangler = match kind {
            ManglerKind::Escape => Box::new(EscapeMangler),
            ManglerKind::Hash => Box::new(HashMangler),
            ManglerKind::Unicode => Box::new(UnicodeMangler),
            ManglerKind::None => Box::new(NoMangler),
        };
        self
    }

    /// Set a custom [`Mangler`].
    ///
    /// The default mangler is [`EscapeMangler`].
    pub fn set_custom_mangler(
        &mut self,
        mangler: impl Mangler + Send + Sync + 'static,
    ) -> &mut Self {
        self.mangler = Box::new(mangler);
        self
    }

    /// Set a custom [`Resolver`] (customize how import paths are translated to WESL modules).
    ///
    ///```rust
    /// # use wesl::{FileResolver, Router, VirtualResolver, Wesl};
    /// // `import runtime::constants::PI` is in a custom module mounted at runtime.
    /// let mut resolver = VirtualResolver::new();
    /// resolver.add_module("package::constants".parse().unwrap(), "const PI = 3.1415; const TAU = PI * 2.0;".into());
    /// let mut router = Router::new();
    /// router.mount_fallback_resolver(FileResolver::new("src/shaders"));
    /// router.mount_resolver("runtime".parse().unwrap(), resolver);
    /// let compiler = Wesl::new("").set_custom_resolver(router);
    /// ```
    ///
    /// # WESL Reference
    /// Both [`FileResolver`] and [`VirtualResolver`] are spec-compliant.
    /// Custom resolvers *must* conform to the constraints described in [`Resolver`].
    pub fn set_custom_resolver<CustomResolver: Resolver>(
        self,
        resolver: CustomResolver,
    ) -> Wesl<CustomResolver> {
        Wesl {
            options: self.options,
            use_sourcemap: self.use_sourcemap,
            mangler: self.mangler,
            resolver,
        }
    }

    /// Enable sourcemapping.
    ///
    /// Turning "on" this option improves the quality of error messages.
    ///
    /// # WESL Reference
    /// Sourcemapping is not yet part of the WESL Specification and does not impact
    /// compliance.
    pub fn use_sourcemap(&mut self, val: bool) -> &mut Self {
        self.use_sourcemap = val;
        self
    }

    /// Enable imports.
    ///
    /// # WESL Reference
    /// Imports is a *mandatory* WESL extension.
    ///
    /// Spec: [`Imports.md`](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/Imports.md)
    pub fn use_imports(&mut self, val: bool) -> &mut Self {
        self.options.imports = val;
        self
    }

    /// Enable conditional translation.
    ///
    /// # WESL Reference
    /// Conditional Compilation is a *mandatory* WESL extension.
    /// Spec: [`ConditionalTranslation.md`](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/ConditionalTranslation.md)
    pub fn use_condcomp(&mut self, val: bool) -> &mut Self {
        self.options.condcomp = val;
        self
    }

    /// Enable generics.
    ///
    /// # WESL Reference
    /// Generics is an *experimental* WESL extension.
    ///
    /// Spec: not yet available.
    #[cfg(feature = "generics")]
    pub fn use_generics(&mut self, val: bool) -> &mut Self {
        self.options.generics = val;
        self
    }
    /// Set a conditional compilation feature flag.
    ///
    /// # WESL Reference
    /// Conditional translation is a *mandatory* WESL extension.
    ///
    /// Spec: [`ConditionalTranslation.md`](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/ConditionalTranslation.md)
    pub fn set_feature(&mut self, feat: &str, val: impl Into<Feature>) -> &mut Self {
        self.options
            .features
            .flags
            .insert(feat.to_string(), val.into());
        self
    }
    /// Set conditional compilation feature flags.
    ///
    /// # WESL Reference
    /// Conditional translation is a *mandatory* WESL extension.
    /// Spec: [`ConditionalTranslation.md`](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/ConditionalTranslation.md)
    pub fn set_features(
        &mut self,
        feats: impl IntoIterator<Item = (impl ToString, impl Into<Feature>)>,
    ) -> &mut Self {
        self.options
            .features
            .flags
            .extend(feats.into_iter().map(|(k, v)| (k.to_string(), v.into())));
        self
    }
    /// Unset a conditional compilation feature flag.
    ///
    /// # WESL Reference
    /// Conditional translation is a *mandatory* WESL extension.
    ///
    /// Spec: [`ConditionalTranslation.md`](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/ConditionalTranslation.md)
    pub fn unset_feature(&mut self, feat: &str) -> &mut Self {
        self.options.features.flags.remove(feat);
        self
    }
    /// Set the behavior for unspecified conditional compilation feature flags.
    ///
    /// Controls what happens when a feature flag is used in shader code but not set with
    /// [`Wesl::set_feature`]. By default it turns off the feature, but it can also be set to leave
    /// it in the code ([`Feature::Keep`]) or to trigger compilation error ([`Feature::Error`]).
    ///
    /// # WESL Reference
    /// Conditional translation is a *mandatory* WESL extension.
    ///
    /// Spec: [`ConditionalTranslation.md`](https://github.com/wgsl-tooling-wg/wesl-spec/blob/main/ConditionalTranslation.md)
    pub fn set_missing_feature_behavior(&mut self, val: impl Into<Feature>) -> &mut Self {
        self.options.features.default = val.into();
        self
    }
    /// Remove unused declarations from the final WGSL output.
    ///
    /// Unused declarations are all declarations not used (directly or indirectly) by any
    /// of the entrypoints (functions marked `@compute`, `@vertex` or `@fragment`) in the
    /// root module.
    ///
    /// see also: [`Wesl::keep_declarations`]
    ///
    /// # WESL Reference
    /// Code stripping is an *optional* WESL extension.
    /// Customizing entrypoints returned by the compiler is explicitly allowed by the spec.
    ///
    /// Spec: not yet available.
    pub fn use_stripping(&mut self, val: bool) -> &mut Self {
        self.options.strip = val;
        self
    }
    /// Transform an output into a simplified WGSL that is better supported by
    /// implementors.
    ///
    /// See also: [`lower`].
    ///
    /// # WESL Reference
    /// Lowering is an *experimental* WESL extension.
    ///
    /// Spec: not yet available.
    pub fn use_lower(&mut self, val: bool) -> &mut Self {
        self.options.lower = val;
        self
    }
    /// If stripping is enabled, specify which root module declarations to keep in the
    /// final WGSL. Function entrypoints are kept by default.
    ///
    /// # WESL Reference
    /// Code stripping is an *optional* WESL extension.
    /// Customizing entrypoints returned by the compiler is explicitly allowed by the
    /// spec.
    ///
    /// Spec: not yet available.
    pub fn keep_declarations(&mut self, keep: Vec<String>) -> &mut Self {
        self.options.keep = Some(keep);
        self
    }
    /// If stripping is enabled, keep all entrypoints in the root WESL module.
    ///
    /// Entrypoints are functions with `@compute`, `@vertex` or `@fragment`.
    /// This is the default. See also: [`Wesl::keep_declarations`].
    ///
    /// # WESL Reference
    /// Code stripping is an *optional* WESL extension.
    /// Customizing entrypoints returned by the compiler is explicitly allowed by the
    /// spec.
    ///
    /// Spec: not yet available.
    pub fn keep_all_entrypoints(&mut self) -> &mut Self {
        self.options.keep = None;
        self
    }

    /// Get a reference to the current [`Resolver`].
    pub fn resolver(&self) -> &R {
        &self.resolver
    }
}

/// The result of [`Wesl::compile`].
///
/// This type contains both the resulting WGSL syntax tree and the sourcemap if
/// [`Wesl`] was invoked with sourcemapping enabled.
///
/// This type implements `Display`, call `to_string()` to get the compiled WGSL.
#[derive(Clone, Default)]
pub struct CompileResult {
    pub syntax: TranslationUnit,
    pub sourcemap: Option<BasicSourceMap>,
    /// A list of absolute paths or packages.
    pub modules: Vec<ModulePath>,
}

impl CompileResult {
    pub fn write_to_file(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        std::fs::write(path, self.to_string())
    }

    /// Write the result in rust's `OUT_DIR`.
    ///
    /// This function is meant to be used in a `build.rs` workflow. The output WGSL will
    /// be accessed with the [`include_wesl`] macro. See the crate documentation for a
    /// usage example.
    ///
    /// # Panics
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

impl Display for CompileResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.syntax.fmt(f)
    }
}

/// The result of [`CompileResult::exec`].
///
/// This type contains both the return value of the function called (if any) and the
/// evaluation context (including bindings).
///
/// This type implements `Display`, call `to_string()` to get the function return value.
#[cfg(feature = "eval")]
pub struct ExecResult<'a> {
    /// The executed function return value
    pub inst: Option<eval::Instance>,
    /// Context after execution
    pub ctx: eval::Context<'a>,
}

#[cfg(feature = "eval")]
impl ExecResult<'_> {
    /// Get the function return value.
    pub fn return_value(&self) -> Option<&eval::Instance> {
        self.inst.as_ref()
    }

    /// Get a [shader resource](https://www.w3.org/TR/WGSL/#resource).
    ///
    /// Shader resources (aka. bindings) with `write`
    /// [access mode](https://www.w3.org/TR/WGSL/#memory-access-mode) can be modified
    /// after executing an entry point.
    pub fn resource(&self, group: u32, binding: u32) -> Option<&eval::RefInstance> {
        self.ctx.resource(group, binding)
    }
}

/// The result of [`CompileResult::eval`].
///
/// This type contains both the resulting WGSL instance and the evaluation context
/// (including bindings).
///
/// This type implements `Display`, call `to_string()` to get the evaluation result.
#[cfg(feature = "eval")]
pub struct EvalResult<'a> {
    /// The expression evaluation result
    pub inst: eval::Instance,
    /// Context after evaluation
    pub ctx: eval::Context<'a>,
}

#[cfg(feature = "eval")]
impl EvalResult<'_> {
    // TODO: make context non-mut
    /// Get the WGSL string representing the evaluated expression.
    pub fn to_buffer(&mut self) -> Option<Vec<u8>> {
        self.inst.to_buffer()
    }
}

#[cfg(feature = "eval")]
impl Display for EvalResult<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inst.fmt(f)
    }
}

#[cfg(feature = "eval")]
impl CompileResult {
    /// Evaluate a const-expression in the context of this compilation result.
    ///
    /// Highly experimental. Not all builtin WGSL functions are supported yet.
    /// Contrary to [`eval_str`], the provided expression can reference declarations
    /// in the compiled WGSL: global const-declarations and user-defined functions with
    /// the `@const` attribute.
    ///
    /// # WESL Reference
    /// The user-defined `@const` attribute is non-standard.
    /// See issue [#46](https://github.com/wgsl-tooling-wg/wesl-spec/issues/46#issuecomment-2389531479).
    pub fn eval<'a>(&'a self, source: &str) -> Result<EvalResult<'a>, Error> {
        let expr = source
            .parse::<syntax::Expression>()
            .map_err(|e| Error::Error(Diagnostic::from(e).with_source(source.to_string())))?;
        let (inst, ctx) = eval(&expr, &self.syntax);
        let inst = inst.map_err(|e| {
            Diagnostic::from(e)
                .with_source(source.to_string())
                .with_ctx(&ctx)
        });

        let inst = if let Some(sourcemap) = &self.sourcemap {
            inst.map_err(|e| Error::Error(e.with_sourcemap(sourcemap)))
        } else {
            inst.map_err(Error::Error)
        }?;

        let res = EvalResult { inst, ctx };
        Ok(res)
    }

    /// Execute an entrypoint in the same way that it would be executed on the GPU.
    ///
    /// Highly experimental.
    ///
    /// # WESL Reference
    /// The `@const` attribute is non-standard.
    pub fn exec<'a>(
        &'a self,
        entrypoint: &str,
        inputs: Inputs,
        bindings: HashMap<(u32, u32), eval::RefInstance>,
        overrides: HashMap<String, eval::Instance>,
    ) -> Result<ExecResult<'a>, Error> {
        let mut ctx = eval::Context::new(&self.syntax);
        ctx.add_bindings(bindings);
        ctx.add_overrides(overrides);
        ctx.set_stage(eval::ShaderStage::Exec);

        let entry_fn = eval::SyntaxUtil::decl_function(ctx.source, entrypoint)
            .ok_or_else(|| EvalError::UnknownFunction(entrypoint.to_string()))?;

        let _ = self.syntax.exec(&mut ctx)?;

        let inst = exec_entrypoint(entry_fn, inputs, &mut ctx).map_err(|e| {
            if let Some(sourcemap) = &self.sourcemap {
                Diagnostic::from(e).with_ctx(&ctx).with_sourcemap(sourcemap)
            } else {
                Diagnostic::from(e).with_ctx(&ctx)
            }
        })?;

        Ok(ExecResult { inst, ctx })
    }
}

impl<R: Resolver> Wesl<R> {
    /// Compile a WESL program from a root file.
    ///
    /// # WESL Reference
    /// Spec: not available yet.
    pub fn compile(&self, root: &ModulePath) -> Result<CompileResult, Error> {
        // TODO
        // root.origin = PathOrigin::Absolute; // we force absolute paths

        if self.use_sourcemap {
            compile_sourcemap(root, &self.resolver, &self.mangler, &self.options)
        } else {
            compile(root, &self.resolver, &self.mangler, &self.options)
        }
    }

    /// Compile a WESL program from a root file and output the result in Rust's `OUT_DIR`.
    ///
    /// This function is meant to be used in a `build.rs` workflow. The output WGSL will
    /// be accessed with the [`include_wesl`] macro. See the crate documentation for a
    /// usage example.
    ///
    /// * The first argument is the path to the root module relative to the base
    ///   directory.
    /// * The second argument is the name of the artifact, used in [`include_wesl`].
    ///
    /// Will emit `rerun-if-changed` instructions so the build script reruns only if the
    /// shader files are modified.
    ///
    /// # Panics
    /// Panics when compilation fails or if the output file cannot be written.
    /// Pretty-prints the WESL error message to stderr.
    pub fn build_artifact(&self, root: &ModulePath, artifact_name: &str) {
        let compiled = self
            .compile(root)
            .inspect_err(|e| {
                eprintln!("failed to build WESL shader `{root}`.\n{e}");
                panic!();
            })
            .unwrap();
        emit_rerun_if_changed(&compiled.modules, &self.resolver);
        compiled.write_artifact(artifact_name);
    }
}

/// What idents to keep from the root module. They should be either:
/// * all named declarations, if `strip` is disabled or `keep_root` is enabled.
/// * `keep` idents that exist, if it is set and `strip` is enabled,
/// * all entrypoints, if `strip` is enabled and `keep` is not set
fn keep_idents(
    wesl: &TranslationUnit,
    keep: &Option<Vec<String>>,
    keep_root: bool,
    strip: bool,
) -> HashSet<Ident> {
    if strip && !keep_root {
        if let Some(keep) = keep {
            wesl.global_declarations
                .iter()
                .filter_map(|decl| {
                    let ident = decl.ident()?;
                    keep.iter()
                        .any(|name| name == &*ident.name())
                        .then_some(ident)
                })
                .collect()
        } else {
            // when stripping is enabled and keep is unset, we keep the entrypoints (default)
            wesl.entry_points().cloned().collect()
        }
    } else {
        // when stripping is disabled, we keep all declarations in the root module.
        wesl.global_declarations
            .iter()
            .filter_map(|decl| decl.ident())
            .collect()
    }
}

fn compile_pre_assembly(
    root: &ModulePath,
    resolver: &impl Resolver,
    opts: &CompileOptions,
) -> Result<(import::Resolutions, HashSet<Ident>), Error> {
    let resolver: Box<dyn Resolver> = if opts.condcomp {
        Box::new(Preprocessor::new(resolver, |wesl| {
            condcomp::run(wesl, &opts.features)?;
            Ok(())
        }))
    } else {
        Box::new(resolver)
    };

    let wesl = resolver.resolve_module(root)?;
    let keep = keep_idents(&wesl, &opts.keep, opts.keep_root, opts.strip);

    let resolutions = if opts.imports {
        if opts.strip && opts.lazy {
            // we can only use the lazy algorithm if stripping is enabled,
            // because lazy will already strip unused modules.
            import::resolve_lazy(&keep, wesl, root, &resolver)?
        } else {
            import::resolve_eager(wesl, root, &resolver)?
        }
    } else {
        import::Resolutions::new(wesl, root.clone())
    };

    if opts.validate {
        for module in resolutions.modules() {
            let module = module.borrow();
            validate_wesl(&module.source).map_err(|d| {
                d.with_module_path(module.path.clone(), resolver.display_name(&module.path))
            })?;
        }
    }

    Ok((resolutions, keep))
}

fn compile_post_assembly(
    wesl: &mut TranslationUnit,
    options: &CompileOptions,
    keep: &HashSet<Ident>,
) -> Result<(), Error> {
    #[cfg(feature = "generics")]
    if options.generics {
        generics::generate_variants(wesl)?;
        generics::replace_calls(wesl)?;
    };
    if options.validate {
        validate_wgsl(wesl)?;
    }
    if options.lower {
        lower(wesl)?;
    }
    if options.strip {
        strip_except(wesl, keep);
    }
    Ok(())
}

/// Low-level version of [`Wesl::compile`].
/// To get a source map, use [`compile_sourcemap`] instead.
pub fn compile(
    root: &ModulePath,
    resolver: &impl Resolver,
    mangler: &impl Mangler,
    options: &CompileOptions,
) -> Result<CompileResult, Error> {
    let (mut resolutions, keep) = compile_pre_assembly(root, resolver, options)?;
    resolutions.mangle(mangler, options.mangle_root);
    let mut assembly = resolutions.assemble(options.strip && options.lazy);
    // resolutions hold idents use-counts. We only need the list of modules now.
    let modules = resolutions.into_module_order();
    compile_post_assembly(&mut assembly, options, &keep)?;
    Ok(CompileResult {
        syntax: assembly,
        sourcemap: None,
        modules,
    })
}

/// Like [`compile`], but provides better error diagnostics and returns the sourcemap.
pub fn compile_sourcemap(
    root: &ModulePath,
    resolver: &impl Resolver,
    mangler: &impl Mangler,
    options: &CompileOptions,
) -> Result<CompileResult, Error> {
    let sourcemapper = SourceMapper::new(root, resolver, mangler);

    match compile_pre_assembly(root, &sourcemapper, options) {
        Ok((mut resolutions, keep)) => {
            resolutions.mangle(&sourcemapper, options.mangle_root);
            let sourcemap = sourcemapper.finish();
            let mut assembly = resolutions.assemble(options.strip && options.lazy);
            let modules = resolutions.into_module_order();
            compile_post_assembly(&mut assembly, options, &keep)
                .map_err(|e| {
                    Diagnostic::from(e)
                        .with_output(assembly.to_string())
                        .with_sourcemap(&sourcemap)
                        .unmangle(Some(&sourcemap), Some(&mangler))
                        .into()
                })
                .map(|()| CompileResult {
                    syntax: assembly,
                    sourcemap: Some(sourcemap),
                    modules,
                })
        }
        Err(e) => {
            let sourcemap = sourcemapper.finish();
            Err(Diagnostic::from(e)
                .with_sourcemap(&sourcemap)
                .unmangle(Some(&sourcemap), Some(&mangler))
                .into())
        }
    }
}

/// Evaluate a const-expression.
///
/// Only builtin function declarations marked `@const` can be called from
/// const-expressions.
///
/// Highly experimental. Not all builtin `@const` WGSL functions are supported yet.
#[cfg(feature = "eval")]
pub fn eval_str(expr: &str) -> Result<eval::Instance, Error> {
    let expr = expr
        .parse::<syntax::Expression>()
        .map_err(|e| Error::Error(Diagnostic::from(e).with_source(expr.to_string())))?;
    let wgsl = TranslationUnit::default();
    let (inst, ctx) = eval(&expr, &wgsl);
    inst.map_err(|e| {
        Error::Error(
            Diagnostic::from(e)
                .with_source(expr.to_string())
                .with_ctx(&ctx),
        )
    })
}

/// Low-level version of [`eval_str`].
#[cfg(feature = "eval")]
pub fn eval<'s>(
    expr: &syntax::Expression,
    wgsl: &'s TranslationUnit,
) -> (Result<eval::Instance, EvalError>, eval::Context<'s>) {
    let mut ctx = eval::Context::new(wgsl);
    let res = wgsl.exec(&mut ctx).and_then(|_| expr.eval(&mut ctx));
    (res, ctx)
}

/// Low-level version of [`CompileResult::exec`].
#[cfg(feature = "eval")]
pub fn exec<'s>(
    expr: &impl Eval,
    wgsl: &'s TranslationUnit,
    bindings: HashMap<(u32, u32), eval::RefInstance>,
    overrides: HashMap<String, eval::Instance>,
) -> (Result<Option<eval::Instance>, EvalError>, eval::Context<'s>) {
    let mut ctx = eval::Context::new(wgsl);
    ctx.add_bindings(bindings);
    ctx.add_overrides(overrides);
    ctx.set_stage(eval::ShaderStage::Exec);

    let res = wgsl.exec(&mut ctx).and_then(|_| match expr.eval(&mut ctx) {
        Ok(ret) => Ok(Some(ret)),
        Err(eval::EvalError::Void(_)) => Ok(None),
        Err(e) => Err(e),
    });
    (res, ctx)
}

#[test]
fn test_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Wesl<StandardResolver>>();
}
