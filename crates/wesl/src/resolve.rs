use crate::{Diagnostic, Error};

use itertools::Itertools;
use wgsl_parse::syntax::{ModulePath, PathOrigin, TranslationUnit};
use wgsl_types::inst::LiteralInstance;

use std::{
    borrow::Cow,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// Error produced by module resolution.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("file not found: `{0}` ({1})")]
    FileNotFound(PathBuf, String),
    #[error("module not found: `{0}` ({1})")]
    ModuleNotFound(ModulePath, String),
    #[error("{0}")]
    Error(#[from] Diagnostic<Error>),
}

type E = ResolveError;

/// A Resolver implements the module resolution algorithm: it returns a module contents
/// associated with a module path.
///
/// Typically implementations of [`Resolver`] only implement [`Resolver::resolve_source`].
///
/// Calls to `Resolver` functions must respect these preconditions:
/// * the import path must not be relative.
pub trait Resolver {
    /// Try to resolve a source file identified by a module path.
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError>;
    /// Try to resolve a source file identified by a module path.
    fn resolve_module(&self, path: &ModulePath) -> Result<TranslationUnit, ResolveError> {
        let source = self.resolve_source(path)?;
        let wesl: TranslationUnit = source.parse().map_err(|e| {
            Diagnostic::from(e)
                .with_module_path(path.clone(), self.display_name(path))
                .with_source(source.to_string())
        })?;
        Ok(wesl)
    }
    /// Get the display name of the module path. Implementing this is optional.
    fn display_name(&self, _path: &ModulePath) -> Option<String> {
        None
    }
    /// Get the filesystem path of the module path. Implementing this is optional.
    /// Used by build scripts for dependency tracking.
    fn fs_path(&self, _path: &ModulePath) -> Option<PathBuf> {
        None
    }
    /// Get the canonical form of a module path.
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        path.clone()
    }
}

impl<T: Resolver + ?Sized> Resolver for Box<T> {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        (**self).resolve_source(path)
    }
    fn resolve_module(&self, path: &ModulePath) -> Result<TranslationUnit, ResolveError> {
        (**self).resolve_module(path)
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        (**self).display_name(path)
    }
    fn fs_path(&self, path: &ModulePath) -> Option<PathBuf> {
        (**self).fs_path(path)
    }
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        (**self).canonical_path(path)
    }
}

impl<T: Resolver> Resolver for &T {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        (**self).resolve_source(path)
    }
    fn resolve_module(&self, path: &ModulePath) -> Result<TranslationUnit, ResolveError> {
        (**self).resolve_module(path)
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        (**self).display_name(path)
    }
    fn fs_path(&self, path: &ModulePath) -> Option<PathBuf> {
        (**self).fs_path(path)
    }
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        (**self).canonical_path(path)
    }
}

/// A resolver that never resolves anything.
///
/// Returns [`ResolveError::ModuleNotFound`] when calling [`Resolver::resolve_source`].
#[derive(Default, Clone, Debug)]
pub struct NoResolver;

impl Resolver for NoResolver {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        Err(E::ModuleNotFound(
            path.clone(),
            "no module resolver, imports are effectively disabled here".to_string(),
        ))
    }
}

/// A resolver that looks for files in the filesystem.
///
/// It simply translates module paths to file paths. This is the intended behavior.
#[derive(Default)]
pub struct FileResolver {
    base: PathBuf,
    extension: &'static str,
}

impl FileResolver {
    /// Create a new resolver.
    ///
    /// `base` is the root directory which absolute paths refer to.
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
            extension: "wesl",
        }
    }

    /// Look for files that ends with a different extension. Default: "wesl".
    pub fn set_extension(&mut self, extension: &'static str) {
        self.extension = extension;
    }

    fn file_path(&self, path: &ModulePath) -> Result<PathBuf, ResolveError> {
        if path.origin.is_package() {
            return Err(E::ModuleNotFound(
                path.clone(),
                "this is an external package import, not a file import. Use `package::` or `super::` for file imports."
                    .to_string(),
            ));
        }
        let mut fs_path = self.base.to_path_buf();
        fs_path.extend(&path.components);
        fs_path.set_extension(self.extension);
        if fs_path.exists() {
            Ok(fs_path)
        } else {
            fs_path.set_extension("wgsl");
            if fs_path.exists() {
                Ok(fs_path)
            } else {
                Err(E::FileNotFound(fs_path, "physical file".to_string()))
            }
        }
    }
}

impl Resolver for FileResolver {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        let fs_path = self.file_path(path)?;
        let source = fs::read_to_string(&fs_path)
            .map_err(|_| E::FileNotFound(fs_path, "physical file".to_string()))?;

        Ok(source.into())
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        self.file_path(path)
            .ok()
            .map(|fs_path| fs_path.display().to_string())
    }
    fn fs_path(&self, path: &ModulePath) -> Option<PathBuf> {
        self.file_path(path).ok()
    }
}

/// A resolver that resolves in-memory modules added with [`Self::add_module`].
///
/// Use-cases are platforms that lack a filesystem (e.g. WASM), tests or
/// runtime-generated files.
#[derive(Default)]
pub struct VirtualResolver<'a> {
    files: HashMap<ModulePath, Cow<'a, str>>,
}

impl<'a> VirtualResolver<'a> {
    /// Create a new resolver.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Resolve imports of `path` with the given WESL string.
    ///
    /// The path must not be relative.
    pub fn add_module(&mut self, path: ModulePath, file: Cow<'a, str>) {
        self.files.insert(path, file);
    }

    /// Resolve imports of `path` with the given [`TranslationUnit`].
    ///
    /// The path must not be relative.
    pub fn add_translation_unit(&mut self, path: ModulePath, translation_unit: TranslationUnit) {
        self.files
            .insert(path, Cow::Owned(translation_unit.to_string()));
    }

    /// Get a module registered with [`Self::add_module`].
    pub fn get_module(&self, path: &ModulePath) -> Result<&str, ResolveError> {
        let source = self
            .files
            .get(path)
            .ok_or_else(|| E::ModuleNotFound(path.clone(), "virtual module".to_string()))?;
        Ok(source)
    }

    /// Iterate over all registered modules.
    pub fn modules(&self) -> impl Iterator<Item = (&ModulePath, &str)> {
        self.files.iter().map(|(res, file)| (res, &**file))
    }
}

impl Resolver for VirtualResolver<'_> {
    fn resolve_source<'b>(&'b self, path: &ModulePath) -> Result<Cow<'b, str>, ResolveError> {
        let source = self.get_module(path)?;
        Ok(source.into())
    }
}

// trait alias
pub trait ResolveFn: Fn(&mut TranslationUnit) -> Result<(), Error> {}
impl<T: Fn(&mut TranslationUnit) -> Result<(), Error>> ResolveFn for T {}

/// A WESL module preprocessor.
///
/// The preprocess function will be called each time the WESL compiler tries to load a
/// module.
pub struct Preprocessor<R: Resolver, F: ResolveFn> {
    pub resolver: R,
    pub preprocess: F,
}

impl<R: Resolver, F: ResolveFn> Preprocessor<R, F> {
    /// Create a new resolver that runs the preprocessing function before each call to
    /// [`Resolver::resolve_module`].
    pub fn new(resolver: R, preprocess: F) -> Self {
        Self {
            resolver,
            preprocess,
        }
    }
}

impl<R: Resolver, F: ResolveFn> Resolver for Preprocessor<R, F> {
    fn resolve_source<'b>(&'b self, path: &ModulePath) -> Result<Cow<'b, str>, ResolveError> {
        let res = self.resolver.resolve_source(path)?;
        Ok(res)
    }
    fn resolve_module(&self, path: &ModulePath) -> Result<TranslationUnit, ResolveError> {
        let mut wesl = self.resolver.resolve_module(path)?;
        (self.preprocess)(&mut wesl).map_err(|e| {
            Diagnostic::from(e)
                .with_module_path(path.clone(), self.display_name(path))
                .with_source(self.resolve_source(path).unwrap().to_string())
        })?;
        Ok(wesl)
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        self.resolver.display_name(path)
    }

    fn fs_path(&self, path: &ModulePath) -> Option<PathBuf> {
        self.resolver.fs_path(path)
    }
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        self.resolver.canonical_path(path)
    }
}

/// A resolver that can dispatch imports to several sub-resolvers based on the import
/// path prefix.
///
/// Add sub-resolvers with [`Self::mount_resolver`].
///
/// This resolver is not thread-safe (not [`Send`] or [`Sync`]).
pub struct Router {
    mount_points: Vec<(ModulePath, Box<dyn Resolver>)>,
    fallback: Option<Box<dyn Resolver>>,
}

/// Dispatches resolution of a module path to sub-resolvers.
///
/// See documentation in [`Self::mount_resolver`]
impl Router {
    /// Create a new resolver.
    pub fn new() -> Self {
        Self {
            mount_points: Vec::new(),
            fallback: None,
        }
    }

    /// Mount a resolver at a given path prefix.
    ///
    /// All import paths starting with `prefix` will be dispatched to the resolver with
    /// the suffix of the path. The prefix path must have an `Absolute` or `Package`
    /// origin and the suffix path will be given an `Absolute` origin.
    ///
    /// If none of the `prefix`es match, the fallback resolver will be used.
    pub fn mount_resolver(&mut self, prefix: ModulePath, resolver: impl Resolver + 'static) {
        self.mount_points.push((prefix, Box::new(resolver)));
    }

    /// Mount a fallback resolver that is used when no other prefix match.
    pub fn mount_fallback_resolver(&mut self, resolver: impl Resolver + 'static) {
        self.fallback = Some(Box::new(resolver));
    }

    fn route(&self, path: &ModulePath) -> Result<(&dyn Resolver, ModulePath), ResolveError> {
        if let Some((mount_path, resolver)) = self
            .mount_points
            .iter()
            .filter(|(prefix, _)| path.starts_with(prefix))
            .max_by_key(|(prefix, _)| prefix.components.len())
        {
            let components = path
                .components
                .iter()
                .skip(mount_path.components.len())
                .cloned()
                .collect_vec();

            let suffix = ModulePath::new(PathOrigin::Absolute, components);
            Ok((resolver, suffix))
        } else if let Some(resolver) = &self.fallback {
            Ok((resolver, path.clone()))
        } else {
            Err(E::ModuleNotFound(
                path.clone(),
                "no mount point".to_string(),
            ))
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver for Router {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        let (resolver, path) = self.route(path)?;
        resolver.resolve_source(&path)
    }
    fn resolve_module(&self, path: &ModulePath) -> Result<TranslationUnit, ResolveError> {
        let (resolver, path) = self.route(path)?;
        resolver.resolve_module(&path)
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        let (resolver, path) = self.route(path).ok()?;
        resolver.display_name(&path)
    }
    fn fs_path(&self, path: &ModulePath) -> Option<PathBuf> {
        let (resolver, path) = self.route(path).ok()?;
        resolver.fs_path(&path)
    }
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        // a rewritten path is in the sub-resolver's namespace
        match self.route(path) {
            Ok((resolver, routed)) if routed == *path => resolver.canonical_path(path),
            _ => path.clone(),
        }
    }
}

/// The type holding the source code of external packages.
///
/// You typically don't implement this, instead it is generated for you by [`crate::PkgBuilder`].
/// Crates containing shader packages export `const` instances of this type, which you can
/// then import and [add to your resolver][StandardResolver::add_package].
#[derive(Debug, PartialEq, Eq)]
pub struct CodegenPkg {
    pub crate_name: &'static str,
    pub root: &'static CodegenModule,
    pub dependencies: &'static [&'static CodegenPkg],
}

/// The type holding the source code of modules in external packages.
///
/// See [`CodegenPkg`].
#[derive(Debug, PartialEq, Eq)]
pub struct CodegenModule {
    pub name: &'static str,
    pub source: &'static str,
    pub submodules: &'static [&'static CodegenModule],
}

/// A resolver that only resolves module paths that refer to modules in external packages.
///
/// Register external packages with [`Self::add_package`].
pub struct PkgResolver {
    packages: Vec<&'static CodegenPkg>,
}

impl PkgResolver {
    /// Create a new resolver.
    pub fn new() -> Self {
        Self {
            packages: Vec::new(),
        }
    }

    /// Add a package to the resolver.
    pub fn add_package(&mut self, pkg: &'static CodegenPkg) {
        self.packages.push(pkg);
    }
}

impl Default for PkgResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver for PkgResolver {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<std::borrow::Cow<'a, str>, E> {
        let pkg_path = match &path.origin {
            PathOrigin::Package(pkg) => pkg,
            _ => {
                return Err(E::ModuleNotFound(
                    path.clone(),
                    "resolver can only resolve package imports".to_string(),
                ));
            }
        };

        // This is a hack: when the package name contains `/`, it corresponds to a sub-dependency
        // of a package dependency. The name is created by the import resolution algorithm.
        // (see ModulePath::join_path)
        let pkg_parts = pkg_path.split('/').collect_vec();

        let root_pkg = pkg_parts
            .first()
            .and_then(|name| self.packages.iter().find(|p| p.root.name == *name))
            .ok_or_else(|| {
                E::ModuleNotFound(
                    path.clone(),
                    format!("dependency `{}` not found", pkg_parts.iter().format("/"),),
                )
            })?;

        let pkg = pkg_parts.iter().skip(1).try_fold(root_pkg, |dep, name| {
            dep.dependencies
                .iter()
                .find(|p| p.root.name == *name)
                .ok_or_else(|| {
                    E::ModuleNotFound(
                        path.clone(),
                        format!(
                            "dependency `{}` not found in package path `{}`",
                            name,
                            pkg_parts.iter().format("/"),
                        ),
                    )
                })
        })?;

        // TODO: the resolution algorithm is currently not spec-compliant.
        // https://github.com/wgsl-tooling-wg/wesl-spec/blob/imports-update/Imports.md
        let mut cur_mod = pkg.root;
        for comp in &path.components {
            if let Some(submod) = cur_mod.submodules.iter().find(|m| m.name == comp) {
                cur_mod = submod;
            } else {
                return Err(E::ModuleNotFound(
                    path.clone(),
                    format!("in module `{}`, no submodule named `{comp}`", cur_mod.name),
                ));
            }
        }
        Ok(cur_mod.source.into())
    }

    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        let PathOrigin::Package(pkg_path) = &path.origin else {
            return path.clone();
        };

        // same package lookup as resolve_source, but failures return the path unchanged
        let pkg_parts = pkg_path.split('/').collect_vec();
        let Some(root_pkg) = pkg_parts
            .first()
            .and_then(|name| self.packages.iter().find(|p| p.root.name == *name))
        else {
            return path.clone();
        };
        let Ok(pkg) = pkg_parts.iter().skip(1).try_fold(root_pkg, |dep, name| {
            dep.dependencies
                .iter()
                .find(|p| p.root.name == *name)
                .ok_or(())
        }) else {
            return path.clone();
        };

        // pick the first name found in BFS order over registered packages
        let mut queue = std::collections::VecDeque::new();
        queue.extend(self.packages.iter().map(|p| (p.root.name.to_string(), *p)));
        let mut visited = Vec::new();
        while let Some((name, candidate)) = queue.pop_front() {
            if std::ptr::eq(candidate, *pkg) {
                return ModulePath::new(PathOrigin::Package(name), path.components.clone());
            }
            if visited.contains(&std::ptr::from_ref(candidate)) {
                continue;
            }
            visited.push(std::ptr::from_ref(candidate));
            for dep in candidate.dependencies {
                queue.push_back((format!("{name}/{}", dep.root.name), *dep));
            }
        }
        path.clone()
    }
}

/// The resolver that implements the WESL standard.
///
/// It resolves modules in external packages registered with [`Self::add_package`] and
/// modules in the local package with the filesystem.
pub struct StandardResolver {
    pkg: PkgResolver,
    files: FileResolver,
    constants: HashMap<String, LiteralInstance>,
}

impl StandardResolver {
    /// Create a new resolver.
    ///
    /// `base` is the root directory which absolute paths refer to.
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            pkg: PkgResolver::new(),
            files: FileResolver::new(base),
            constants: HashMap::new(),
        }
    }

    /// Add an external package.
    pub fn add_package(&mut self, pkg: &'static CodegenPkg) {
        self.pkg.add_package(pkg)
    }

    /// Add a numeric constant.
    ///
    /// Numeric constants live WESL's special package named `constants`. This package is
    /// *virtual*, meaning it doesn't exist on the filesystem. Constants can be accessed
    /// by importing them: `import constants::MY_CONSTANT;`.
    ///
    /// The type is specified by the variant of [`LiteralInstance`].\
    /// If specifying a constant that is used with multiple different types or
    /// a constant that benefits from precision, like π, use AbstractFloat,
    /// which can be implicitly converted to all scalar types.
    ///
    /// Note: [`LiteralInstance`] implements [`From`] for all standard numeric types
    pub fn add_constant(&mut self, name: impl ToString, value: LiteralInstance) {
        self.constants.insert(name.to_string(), value);
    }

    /// Generate a module with all declared virtual constants in the resolver
    fn generate_constant_module(&self) -> String {
        self.constants
            .iter()
            .map(|(name, value)| format!("const {name} = {value};"))
            .join("\n")
    }
}

impl Resolver for StandardResolver {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        // a special case to handle the constants virtual module. For now, this module
        // is shared for all sub-dependencies.
        // TODO: in the future we'll change that.
        if let PathOrigin::Package(pkg_name) = &path.origin
            && (pkg_name == "constants" || pkg_name.ends_with("/constants"))
        {
            return Ok(self.generate_constant_module().into());
        }

        if path.origin.is_package() {
            self.pkg.resolve_source(path)
        } else {
            self.files.resolve_source(path)
        }
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        if path.origin.is_package() {
            self.pkg.display_name(path)
        } else {
            self.files.display_name(path)
        }
    }
    fn fs_path(&self, path: &ModulePath) -> Option<PathBuf> {
        if path.origin.is_package() {
            self.pkg.fs_path(path)
        } else {
            self.files.fs_path(path)
        }
    }
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        // the constants module is shared for all sub-dependencies
        if let PathOrigin::Package(pkg_name) = &path.origin
            && (pkg_name == "constants" || pkg_name.ends_with("/constants"))
        {
            return ModulePath::new(
                PathOrigin::Package("constants".to_string()),
                path.components.clone(),
            );
        }

        if path.origin.is_package() {
            self.pkg.canonical_path(path)
        } else {
            path.clone()
        }
    }
}

pub fn emit_rerun_if_changed(modules: &[ModulePath], resolver: &impl Resolver) {
    for module in modules {
        if module.origin.is_package() {
            continue;
        }
        assert!(
            !module.origin.is_relative(),
            "the modules passed to emit_rerun_if_changed must be absolute"
        );
        if let Some(mut path) = resolver.fs_path(module) {
            // Path::display is safe here because of the ModulePath naming restrictions
            println!("cargo::rerun-if-changed={}", path.display());

            // If it's a fallback path, we need to react to the higher priority path as well
            if path.extension().unwrap() == "wgsl" {
                path.set_extension("wesl");
                println!("cargo::rerun-if-changed={}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn router_resolver() {
        let mut r = Router::new();

        let mut v1 = VirtualResolver::new();
        v1.add_module("package".parse().unwrap(), "m1".into());
        v1.add_module("package::foo".parse().unwrap(), "m2".into());
        v1.add_module("package::bar".parse().unwrap(), "m3".into());
        r.mount_resolver("package".parse().unwrap(), v1);

        let mut v2 = VirtualResolver::new();
        v2.add_module("package".parse().unwrap(), "m4".into());
        v2.add_module("package::baz".parse().unwrap(), "m5".into());
        r.mount_resolver("package::bar".parse().unwrap(), v2);

        let mut v3 = VirtualResolver::new();
        v3.add_module("foo::bar".parse().unwrap(), "m6".into());
        r.mount_fallback_resolver(v3);

        assert_eq!(r.resolve_source(&"package".parse().unwrap()).unwrap(), "m1");
        assert_eq!(
            r.resolve_source(&"package::foo".parse().unwrap()).unwrap(),
            "m2"
        );
        assert_eq!(
            r.resolve_source(&"package::bar".parse().unwrap()).unwrap(),
            "m4"
        );
        assert_eq!(
            r.resolve_source(&"package::bar::baz".parse().unwrap())
                .unwrap(),
            "m5"
        );
        assert_eq!(
            r.resolve_source(&"foo::bar".parse().unwrap()).unwrap(),
            "m6"
        );
    }

    #[test]
    fn canonical_paths() {
        static C_ROOT: CodegenModule = CodegenModule {
            name: "c",
            source: "",
            submodules: &[],
        };
        static C: CodegenPkg = CodegenPkg {
            crate_name: "c",
            root: &C_ROOT,
            dependencies: &[],
        };
        static D1_ROOT: CodegenModule = CodegenModule {
            name: "d",
            source: "",
            submodules: &[],
        };
        static D1: CodegenPkg = CodegenPkg {
            crate_name: "d",
            root: &D1_ROOT,
            dependencies: &[],
        };
        static D2_ROOT: CodegenModule = CodegenModule {
            name: "d",
            source: "",
            submodules: &[],
        };
        static D2: CodegenPkg = CodegenPkg {
            crate_name: "d",
            root: &D2_ROOT,
            dependencies: &[],
        };
        static A_ROOT: CodegenModule = CodegenModule {
            name: "a",
            source: "",
            submodules: &[],
        };
        static A: CodegenPkg = CodegenPkg {
            crate_name: "a",
            root: &A_ROOT,
            dependencies: &[&C, &D1],
        };
        static B_ROOT: CodegenModule = CodegenModule {
            name: "b",
            source: "",
            submodules: &[],
        };
        static B: CodegenPkg = CodegenPkg {
            crate_name: "b",
            root: &B_ROOT,
            dependencies: &[&C, &D2],
        };
        static E_ROOT: CodegenModule = CodegenModule {
            name: "e",
            source: "",
            submodules: &[],
        };
        static E: CodegenPkg = CodegenPkg {
            crate_name: "e",
            root: &E_ROOT,
            dependencies: &[&C],
        };

        let mut r = PkgResolver::new();
        r.add_package(&A);
        r.add_package(&B);
        r.add_package(&E);

        let canon = |s: &str| r.canonical_path(&s.parse().unwrap());

        // a and b depend on the same instance of c
        assert_eq!(canon("a/c::foo"), canon("b/c::foo"));
        assert_eq!(canon("a/c::foo"), "a/c::foo".parse().unwrap());

        // e reaches the same static through its own dependency
        assert_eq!(canon("e/c::foo"), "a/c::foo".parse().unwrap());

        // two versions of d
        assert_eq!(canon("a/d::foo"), "a/d::foo".parse().unwrap());
        assert_eq!(canon("b/d::foo"), "b/d::foo".parse().unwrap());

        assert_eq!(canon("a::foo"), "a::foo".parse().unwrap());

        assert_eq!(canon("z/c::foo"), "z/c::foo".parse().unwrap());
    }

    #[test]
    /// Test WGSL type casting of virtual constants
    fn type_virtual_constants() {
        // standard resolver to register some constants
        let mut std = StandardResolver::new(".");
        // AbstractFloat
        std.add_constant("TAU", std::f64::consts::TAU.into());
        // f32
        std.add_constant("LIGHTING_ANGLE", 10.0f32.into());
        // i32
        std.add_constant("Z_ROTATION", (-10i32).into());
        // u32
        std.add_constant("H", (12u32).into());
        // bool
        std.add_constant("BRIGHTEN", (false).into());

        // use virtual resolver for the main module
        let mut v = VirtualResolver::new();
        v.add_module(
            "package::color_math".parse().unwrap(),
            // the main module imports constants::TAU and uses it in a context that requires f32,
            // therfor it will be cast from AbstractFloat
            r#"
            import constants::{TAU, H, BRIGHTEN};

            fn color_sweep(h: u32) -> f32 {
                let color = cos(h + vec3(0.0, 1.0, 2.0) * TAU / 3.0);
                if (BRIGHTEN) {
                    color += 0.1;
                }

                return color;
            }

            @fragment
            fn fragment() -> @location(0) vec4<f32> {
                return vec4(color_sweep(H), color_sweep(H + 0.1), color_sweep(H + 0.2), 1.0);
            }
            "#
            .into(),
        );

        // route package imports whose prefix is "constants" to the StandardResolver
        // and absolute module paths to the VirtualResolver.
        let mut r = Router::new();
        r.mount_resolver(ModulePath::new_root(), v);
        r.mount_fallback_resolver(std);

        // compile to test imports and casting
        crate::Wesl::new(".")
            .set_custom_resolver(r)
            .compile(&"package::color_math".parse().unwrap())
            .unwrap();
    }

    #[test]
    /// Test resolving virtual constants from `add_constant`
    fn resolve_virtual_constants() {
        // todo impl `add_constant` for VirtualResolver then use that
        let mut sr = StandardResolver::new(".");

        // add math constants
        sr.add_constant("PI", LiteralInstance::from(std::f64::consts::PI));
        sr.add_constant("E", LiteralInstance::from(std::f64::consts::E));
        // add misc constants
        sr.add_constant("NEG_2", LiteralInstance::from(-2i32));
        sr.add_constant("ONE", LiteralInstance::from(1u32));
        sr.add_constant("F32_MAX", LiteralInstance::from(f32::MAX));
        sr.add_constant("IS_HEAVY", LiteralInstance::from(false));
        sr.add_constant(
            "NUM_CONSTS",
            LiteralInstance::from(sr.constants.len() as i64),
        );

        // generate the virtual module
        let generated = sr.generate_constant_module();
        // test that it contains the consts with correct values
        assert!(generated.contains(&format!("const PI = {:?};", std::f64::consts::PI)));
        assert!(generated.contains(&format!("const E = {:?};", std::f64::consts::E)));
        assert!(generated.contains("const NEG_2 = -2i;"));
        assert!(generated.contains("const ONE = 1u;"));
        assert!(generated.contains(&format!("const F32_MAX = {}f;", f32::MAX)));
        assert!(generated.contains("const IS_HEAVY = false;"));
        assert!(generated.contains(&format!(
            "const NUM_CONSTS = {};",
            (sr.constants.len() as i64) - 1
        )));

        // resolve the package path with the origin `constants`,
        // the source of which should be the same as the generated module
        let src_root = sr.resolve_source(&"constants".parse().unwrap()).unwrap();
        assert_eq!(src_root, generated);

        // resolving a path with components should return the same
        let src_comp = sr
            .resolve_source(&"constants::PI".parse().unwrap())
            .unwrap();
        assert_eq!(src_comp, generated);
    }
}
