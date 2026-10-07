//! [`SourceMap`] trait and implementations.

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Display},
    ops::Range,
    path::PathBuf,
    sync::Arc,
};

use wgsl_parse::{
    PrintedSpan, SyntaxNode, print_with_spans,
    span::Span,
    syntax::{GlobalDeclaration, GlobalDeclarationNode, TranslationUnit, TypeExpression},
};

use crate::{ModulePath, error::ResolveError, mangler::Mangler, pass::Module, resolver::Resolver};

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
    /// Replaces the mangled names in `text` by the paths they come from.
    fn demangle_message(&self, text: &str) -> String {
        let is_separator = |c: char| !(c.is_alphanumeric() || c == '_');
        let mut result = String::with_capacity(text.len());

        for piece in text.split_inclusive(is_separator) {
            let word = piece.trim_end_matches(is_separator);
            let separator = &piece[word.len()..];

            if let Some(entry) = self.item(word).filter(|e| e.name != word) {
                result.push_str(&entry.path.to_string());
                result.push_str("::");
                result.push_str(&entry.name);
                result.push_str(separator);
            } else {
                result.push_str(piece);
            }
        }

        result
    }
    /// Translates a byte range of the compiled code to the innermost syntax node that contains it.
    ///
    /// The range is exact if the node was printed as written, and widened to the node otherwise.
    /// Returns `None` unless the map was finished with the compiled code.
    fn destination_to_source(&self, _span: Range<usize>) -> Option<SourceLocation> {
        None
    }
    /// Creates a diagnostic from a message and labeled ranges of the compiled code.
    ///
    /// All labels are marked primary, as naga does.
    fn diagnostic(
        &self,
        message: impl AsRef<str>,
        labels: impl IntoIterator<Item = (Range<usize>, String)>,
    ) -> MappedDiagnostic
    where
        Self: Sized,
    {
        MappedDiagnostic {
            labels: labels
                .into_iter()
                .map(|(emitted, text)| MappedLabel {
                    location: self.destination_to_source(emitted.clone()),
                    emitted,
                    text: self.demangle_message(&text),
                })
                .collect(),
            message: self.demangle_message(message.as_ref()),
            notes: Vec::new(),
        }
    }
    /// Like [`Self::diagnostic`], with the chain of causes of `error` added as notes.
    ///
    /// Each cause is a note of its own, shown without a prefix as naga shows them.
    fn diagnostic_from_error(
        &self,
        error: &(dyn std::error::Error + 'static),
        labels: impl IntoIterator<Item = (Range<usize>, String)>,
    ) -> MappedDiagnostic
    where
        Self: Sized,
    {
        let mut diagnostic = self.diagnostic(error.to_string(), labels);
        let mut source = error.source();
        while let Some(cause) = source {
            diagnostic
                .notes
                .push(self.demangle_message(&cause.to_string()));
            source = cause.source();
        }
        diagnostic
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
    pub(crate) span_map: Option<SpanMap>,
}

impl BasicSourceMap {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn add_item(&mut self, decl: String, entry: SourceMapEntry) {
        self.mappings.insert(decl, entry);
    }
    /// Iterates over the mangled names and the declarations they come from.
    pub fn items(&self) -> impl Iterator<Item = (&str, &SourceMapEntry)> {
        self.mappings.iter().map(|(k, v)| (k.as_str(), v))
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

    /// Prints `syntax` and records where the nodes of each module in `modules` ended up.
    fn build_span_map(&self, syntax: &TranslationUnit, modules: &[Module]) -> SpanMap {
        let (emitted, printed) = print_with_spans(syntax);

        let declarations = syntax
            .global_declarations
            .iter()
            .filter(|decl| !matches!(decl.node(), GlobalDeclaration::Void))
            .collect::<Vec<_>>();
        let roots = printed.iter().filter(|p| p.depth == 0).collect::<Vec<_>>();
        let roots = &roots[roots.len().saturating_sub(declarations.len())..];

        let mut files = Vec::new();
        let mut regions: Vec<(Range<usize>, usize)> = Vec::new();
        for (root, declaration) in roots.iter().zip(&declarations) {
            let module = modules.iter().find(|module| {
                let linked = &module.syntax.global_declarations;
                linked
                    .iter()
                    .any(|decl| Self::is_copy_of(decl, declaration))
            });
            let file = module.and_then(|m| self.file_index(&mut files, &m.path));
            if let (Some(file), true) = (file, root.source_span == declaration.span()) {
                regions.push((root.range.clone(), file));
            }
        }

        let nodes = printed
            .iter()
            .filter_map(|p| self.node(p, &regions, &files, &emitted))
            .collect();
        SpanMap {
            emitted: emitted.into(),
            files,
            nodes,
        }
    }

    /// Returns the index of the module `path` in `files`, adding it if its source is known.
    fn file_index(&self, files: &mut Vec<ModulePath>, path: &ModulePath) -> Option<usize> {
        if let Some(index) = files.iter().position(|file| file == path) {
            return Some(index);
        }
        self.source(path)?;
        files.push(path.clone());
        Some(files.len() - 1)
    }

    /// Builds the node for a printed span, if its file and original text are known.
    fn node(
        &self,
        printed: &PrintedSpan,
        regions: &[(Range<usize>, usize)],
        files: &[ModulePath],
        emitted: &str,
    ) -> Option<Node> {
        let (_, file) = regions.iter().find(|(range, _)| {
            range.start <= printed.range.start && printed.range.end <= range.end
        })?;
        if printed.source_span.range().is_empty() {
            return None;
        }
        let orig_text = self
            .source(&files[*file])?
            .get(printed.source_span.range())?;
        Some(Node {
            emitted: printed.range.clone(),
            exact: emitted.get(printed.range.clone()) == Some(orig_text),
            file: *file,
            orig: printed.source_span.range(),
        })
    }

    /// Returns whether `linked` is the linked copy of `declaration`.
    fn is_copy_of(linked: &GlobalDeclarationNode, declaration: &GlobalDeclarationNode) -> bool {
        match (linked.ident(), declaration.ident()) {
            (Some(a), Some(b)) => a == b,
            (None, None) => linked.span() == declaration.span() && linked == declaration,
            _ => false,
        }
    }

    /// Returns the 1-based line and column of the byte `offset` in `source`.
    fn line_col(source: &str, offset: usize) -> (usize, usize) {
        let before = source.get(..offset).unwrap_or(source);
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        (line, column)
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
    fn destination_to_source(&self, span: Range<usize>) -> Option<SourceLocation> {
        let span_map = self.span_map.as_ref()?;
        let node = span_map
            .nodes
            .iter()
            .filter(|n| n.emitted.start <= span.start && span.end <= n.emitted.end)
            .min_by_key(|n| n.emitted.len())?;
        let module = &span_map.files[node.file];
        let source = self.source(module)?;

        let shifted = node.exact.then(|| {
            let offset = span.start - node.emitted.start;
            node.orig.start + offset..node.orig.start + offset + span.len()
        });
        let (range, exact) = match shifted {
            Some(range) if source.get(range.clone()).is_some() => (range, true),
            _ => (node.orig.clone(), false),
        };

        let (line, column) = Self::line_col(source, range.start);
        Some(SourceLocation {
            column,
            exact,
            file: self
                .display_name(module)
                .map_or_else(|| module.to_string(), str::to_string),
            line,
            module: module.clone(),
            source: source.into(),
            span: range,
        })
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
    fn destination_to_source(&self, span: Range<usize>) -> Option<SourceLocation> {
        self.as_ref()
            .and_then(|map| map.destination_to_source(span))
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
    /// Like [`Self::finish`], and records the span map of the compiled code of `syntax`.
    ///
    /// Returns the map and the compiled code. `modules` are the modules `syntax` was linked from.
    pub fn finish_with_output(
        self,
        syntax: &TranslationUnit,
        modules: &[Module],
    ) -> (BasicSourceMap, Arc<str>) {
        let mut sourcemap = self.finish();
        let span_map = sourcemap.build_span_map(syntax, modules);
        let wgsl = span_map.emitted.clone();
        sourcemap.span_map = Some(span_map);
        (sourcemap, wgsl)
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
                path: self.resolver.fs_path(path).ok(),
            },
        );
        Ok(res)
    }
    fn display_name(&self, path: &ModulePath) -> Option<String> {
        self.resolver.display_name(path)
    }
    fn fs_path(&self, path: &ModulePath) -> Result<PathBuf, ResolveError> {
        self.resolver.fs_path(path)
    }
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        self.resolver.canonical_path(path)
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

/// An error whose labels point at the original sources.
#[derive(Clone, Debug)]
pub struct MappedDiagnostic {
    /// The labeled ranges.
    pub labels: Vec<MappedLabel>,
    /// The message of the error.
    pub message: String,
    /// The notes displayed after the snippets, each of them as written.
    pub notes: Vec<String>,
}

impl MappedDiagnostic {
    /// Renders the diagnostic with `renderer`, with one snippet per original file.
    fn render(&self, renderer: &annotate_snippets::Renderer) -> String {
        use annotate_snippets::*;

        let title = Level::ERROR.primary_title(&self.message);
        let mut group = Group::with_title(title);

        let mut modules: Vec<&ModulePath> = Vec::new();
        for location in self.labels.iter().filter_map(|l| l.location.as_ref()) {
            if !modules.contains(&&location.module) {
                modules.push(&location.module);
            }
        }
        for module in modules {
            let labels = self.labels.iter().filter_map(|label| {
                let location = label.location.as_ref().filter(|l| l.module == *module)?;
                Some((label, location))
            });
            let labels = labels.collect::<Vec<_>>();
            let first = labels[0].1;
            let mut snippet = Snippet::source(first.source()).path(&first.file).fold(true);
            for (label, location) in labels {
                let annotation = annotate(location.span.clone(), &label.text);
                snippet = snippet.annotation(annotation);
            }
            group = group.element(snippet);
        }

        for label in self.labels.iter().filter(|l| l.location.is_none()) {
            let text = format!("{} (generated code {:?})", label.text, label.emitted);
            group = group.element(Level::NOTE.message(text));
        }

        for note in &self.notes {
            group = group.element(Level::NOTE.no_name().message(note));
        }
        renderer.render(&[group])
    }

    /// Renders the diagnostic with ANSI colors.
    pub fn render_colored(&self) -> String {
        self.render(&annotate_snippets::Renderer::styled())
    }

    /// Renders the diagnostic without colors.
    pub fn render_plain(&self) -> String {
        self.render(&annotate_snippets::Renderer::plain())
    }

    /// Adds a note displayed after the snippets as `note: ...`.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(format!("note: {}", note.into()));
        self
    }
}

impl Display for MappedDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render_plain())
    }
}

impl std::error::Error for MappedDiagnostic {}

/// A label of a [`MappedDiagnostic`].
#[derive(Clone, Debug)]
pub struct MappedLabel {
    /// The range in the compiled code, as reported by the tool.
    pub emitted: Range<usize>,
    /// The location in the original source, if the range could be resolved.
    pub location: Option<SourceLocation>,
    /// The description of the label.
    pub text: String,
}

/// A syntax node found in both the compiled code and an original source.
#[derive(Clone, Debug)]
pub(crate) struct Node {
    /// The range of the node in the compiled code.
    pub emitted: Range<usize>,
    /// Whether the compiled text of the node equals its original text.
    pub exact: bool,
    /// The index of the module the node comes from in [`SpanMap::files`].
    pub file: usize,
    /// The range of the node in the original source.
    pub orig: Range<usize>,
}

/// A location in an original source file.
#[derive(Clone, Debug)]
pub struct SourceLocation {
    /// The 1-based column, in characters, the range starts at.
    pub column: usize,
    /// Whether the range is exact, or was widened to the enclosing syntax node.
    pub exact: bool,
    /// The name of the file, usually its path.
    pub file: String,
    /// The 1-based line the range starts on.
    pub line: usize,
    /// The module the location is in.
    pub module: ModulePath,
    /// The full text of the original source.
    source: Arc<str>,
    /// The byte range in the original source.
    pub span: Range<usize>,
}

impl SourceLocation {
    /// The text of the original source covered by [`Self::span`].
    pub fn snippet(&self) -> &str {
        self.source.get(self.span.clone()).unwrap_or_default()
    }

    /// The full text of the original source file.
    pub fn source(&self) -> &str {
        &self.source
    }
}

impl Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// The compiled code and the ranges of the syntax nodes it was printed from.
///
/// It is recorded when a [`SourceMapper`] finishes with the compiled code.
#[derive(Clone, Debug)]
pub(crate) struct SpanMap {
    /// The compiled code the nodes were recorded from.
    pub emitted: Arc<str>,
    /// The modules that contributed code.
    pub files: Vec<ModulePath>,
    /// The nodes of the compiled code, children before their parents.
    pub nodes: Vec<Node>,
}

/// Creates a primary annotation of `span` with `text` as its label, if there is one.
fn annotate(span: Range<usize>, text: &str) -> annotate_snippets::Annotation<'_> {
    let annotation = annotate_snippets::AnnotationKind::Primary.span(span);
    if text.is_empty() {
        annotation
    } else {
        annotation.label(text)
    }
}
