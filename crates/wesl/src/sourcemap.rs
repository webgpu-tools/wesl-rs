//! [`SourceMap`] trait and implementations.

use std::{cell::RefCell, collections::HashMap, path::PathBuf};

use wgsl_parse::{span::Span, syntax::TypeExpression};

use crate::{ModulePath, error::ResolveError, mangler::Mangler, resolver::Resolver};

/// A SourceMap is a lookup from compiled WGSL to source WESL. It translates a mangled
/// name into a module path and declaration name.
///
/// Using SourceMaps improves the readability of error diagnostics, by providing needed
/// information to identify the originating code snippet, file name and declaration name.
/// It is highly recommended to use them, but they can increase the compilation memory
/// footprint, since they cache all loaded files.
///
/// Typically you record to a SourceMap by passing a [`SourceMapper`] as the [`Resolver`]
/// and [`Mangler`] when compiling code.
pub trait SourceMap {
    /// Get the module path and declaration name from a mangled name.
    fn item(&self, decl: &str) -> Option<&SourceMapEntry>;
    /// Get a module contents.
    fn source(&self, path: &ModulePath) -> Option<&str>;
    /// Get a module display name.
    fn display_name(&self, path: &ModulePath) -> Option<&str>;
    /// Get the default module contents.
    fn default_source(&self) -> Option<&str> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct SourceMapEntry {
    pub path: ModulePath,
    pub name: String,
    pub span: Option<Span>,
}

#[derive(Clone, Debug)]
pub struct SourceMapFile {
    pub source: String,
    pub display_name: Option<String>,
    pub path: Option<PathBuf>,
}

/// Basic implementation of [`SourceMap`].
#[derive(Clone, Debug, Default)]
pub struct BasicSourceMap {
    mappings: HashMap<String, SourceMapEntry>,
    sources: HashMap<ModulePath, SourceMapFile>,
    default_source: Option<String>,
}

impl BasicSourceMap {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn add_item(&mut self, decl: String, entry: SourceMapEntry) {
        self.mappings.insert(decl, entry);
    }
    pub fn file(&self, path: &ModulePath) -> Option<&SourceMapFile> {
        self.sources.get(path)
    }
    pub fn add_file(&mut self, path: ModulePath, file: SourceMapFile) {
        self.sources.insert(path, file);
    }
    pub fn set_default_source(&mut self, source: String) {
        self.default_source = Some(source);
    }
}

impl SourceMap for BasicSourceMap {
    fn item(&self, decl: &str) -> Option<&SourceMapEntry> {
        self.mappings.get(decl)
    }
    fn source(&self, path: &ModulePath) -> Option<&str> {
        self.sources.get(path).map(|file| file.source.as_str())
    }
    fn display_name(&self, path: &ModulePath) -> Option<&str> {
        self.sources
            .get(path)
            .and_then(|file| file.display_name.as_deref())
    }
    fn default_source(&self) -> Option<&str> {
        self.default_source.as_deref()
    }
}

impl<T: SourceMap> SourceMap for Option<T> {
    fn item(&self, decl: &str) -> Option<&SourceMapEntry> {
        self.as_ref().and_then(|map| map.item(decl))
    }
    fn source(&self, path: &ModulePath) -> Option<&str> {
        self.as_ref().and_then(|map| map.source(path))
    }
    fn display_name(&self, path: &ModulePath) -> Option<&str> {
        self.as_ref().and_then(|map| map.display_name(path))
    }
    fn default_source(&self) -> Option<&str> {
        self.as_ref().and_then(|map| map.default_source())
    }
}

/// This [`SourceMap`] implementation simply does nothing and returns `None`.
///
/// It can be useful to pass this struct to functions requiring a sourcemap, but
/// you don't care about sourcemapping.
pub struct NoSourceMap;

impl SourceMap for NoSourceMap {
    fn item(&self, _decl: &str) -> Option<&SourceMapEntry> {
        None
    }
    fn source(&self, _path: &ModulePath) -> Option<&str> {
        None
    }
    fn display_name(&self, _path: &ModulePath) -> Option<&str> {
        None
    }
    fn default_source(&self) -> Option<&str> {
        None
    }
}

/// Generate a SourceMap by keeping track of loaded files and mangled identifiers.
///
/// `SourceMapper` is a proxy that implements [`Mangler`] and [`Resolver`]. To record a
/// SourceMap, invoke the compiler with this instance as both the mangler and the
/// resolver. Call [`SourceMapper::finish`] to get the final SourceMap once finished
/// recording.
pub struct SourceMapper<'a> {
    pub main_path: ModulePath,
    pub resolver: &'a dyn Resolver,
    pub mangler: &'a dyn Mangler,
    pub sourcemap: RefCell<BasicSourceMap>,
}

impl<'a> SourceMapper<'a> {
    /// Create a new `SourceMapper` from a mangler and a resolver.
    pub fn new(
        main_path: ModulePath,
        resolver: &'a dyn Resolver,
        mangler: &'a dyn Mangler,
    ) -> Self {
        Self {
            main_path,
            resolver,
            mangler,
            sourcemap: Default::default(),
        }
    }
    /// Consume this and return a [`BasicSourceMap`].
    pub fn finish(self) -> BasicSourceMap {
        let mut sourcemap = self.sourcemap.into_inner();
        if let Some(file) = sourcemap.file(&self.main_path) {
            sourcemap.set_default_source(file.source.to_string());
        }
        sourcemap
    }
}

impl<'a> Resolver for SourceMapper<'a> {
    fn resolve_source(&self, path: &ModulePath) -> Result<std::borrow::Cow<'a, str>, ResolveError> {
        let res = self.resolver.resolve_source(path)?;
        let mut sourcemap = self.sourcemap.borrow_mut();
        sourcemap.add_file(
            path.clone(),
            SourceMapFile {
                source: res.clone().into(),
                display_name: self.resolver.display_name(path),
                path: self.resolver.fs_path(path),
            },
        );
        Ok(res)
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        self.resolver.display_name(path)
    }
    fn fs_path(&self, path: &ModulePath) -> Option<PathBuf> {
        self.resolver.fs_path(path)
    }
}

impl<'a> Mangler for SourceMapper<'a> {
    fn mangle(&self, path: &ModulePath, item: &str) -> String {
        let res = self.mangler.mangle(path, item);
        let mut sourcemap = self.sourcemap.borrow_mut();
        let entry = SourceMapEntry {
            path: path.clone(),
            name: item.to_string(),
            span: None,
        };
        sourcemap.add_item(res.clone(), entry);
        res
    }
    fn unmangle(&self, mangled: &str) -> Option<(ModulePath, String)> {
        self.mangler.unmangle(mangled)
    }
    fn mangle_types(&self, item: &str, variant: u32, types: &[TypeExpression]) -> String {
        self.mangler.mangle_types(item, variant, types)
    }
}
