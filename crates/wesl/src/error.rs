//! [`enum@Error`] types.

use std::{fmt::Display, path::PathBuf};

use thiserror::Error;
use wgsl_parse::{
    span::Span,
    syntax::{Expression, Ident, ModulePath, Visibility},
};
use wgsl_types::ty_ctx::{DisplayWithContext, TyContext};

#[cfg(feature = "eval")]
use crate::eval::EvalError;
use crate::{Mangler, sourcemap::SourceMap};

/// Conditional translation error.
#[derive(Debug, Error)]
pub enum CondCompError {
    #[error("invalid feature flag: `{0}`")]
    InvalidFeatureFlag(String),
    #[error("unexpected feature flag: `{0}`")]
    UnexpectedFeatureFlag(String),
    #[error("invalid if attribute expression: `{0}`")]
    InvalidExpression(Expression),
    #[error("an @elif or @else attribute must be preceded by a @if or @elif on the previous node")]
    NoPrecedingIf,
    #[error("cannot have multiple @if/@elif/@else attributes on the same node")]
    DuplicateIf,
}

/// Error produced by module resolution.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("I/O Error: {0}")]
    Io(std::io::Error),
    #[error("file not found: `{0}` ({1})")]
    FileNotFound(PathBuf, String),
    #[error("module not found: `{0}` ({1})")]
    ModuleNotFound(ModulePath, String),
    #[error("the resolver does not support file system paths")]
    FilesystemNotSupported,
    #[error("attempt to access path `{0}`, which escapes the package root path `{1}`")]
    FileEscapesRoot(PathBuf, PathBuf),
    #[error("{0}")]
    Custom(String),
}

/// WESL or WGSL Validation error.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ValidateError {
    #[error("cannot find declaration of `{0}`")]
    UndefinedSymbol(String),
    #[error("incorrect number of arguments to `{0}`, expected `{1}`, got `{2}`")]
    ParamCount(String, usize, usize),
    #[error("`{0}` is not callable")]
    NotCallable(String),
    #[error("duplicate declaration of `{0}`")]
    Duplicate(String),
    #[error("declaration of `{0}` is cyclic via `{1}`")]
    Cycle(String, String),
}

/// Error produced during import resolution.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("duplicate declaration of `{0}`")]
    DuplicateSymbol(String),
    #[error("{0}")]
    ResolveError(#[from] ResolveError),
}

#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("module `{0}` has no declaration `{1}`")]
    NotFound(ModulePath, String),
    #[error(
        "`{decl_path}::{decl_name}` is declared with `{decl_vis}` visibility, but {orig} imports it with `{min_vis}` visibility",
        decl_path = .decl.0,
        decl_name = .decl.1,
        orig = .orig
            .as_ref()
            .map(|(path, name)| format!("`{path}::{name}`"))
            .unwrap_or("another declaration".to_string())
    )]
    Visibility {
        orig: Option<(ModulePath, Ident)>,
        decl: (ModulePath, Ident),
        min_vis: Visibility,
        decl_vis: Visibility,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum TomlError {
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

/// Any WESL error.
#[derive(Debug)]
pub enum Error {
    ParseError(wgsl_parse::Error),
    ValidateError(ValidateError),
    ResolveError(ResolveError),
    ImportError(ImportError),
    UsageError(UsageError),
    CondCompError(CondCompError),
    TomlError(TomlError),
    #[cfg(feature = "eval")]
    EvalError(EvalError, Box<TyContext>),
    Custom(String),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ParseError(err) => err.fmt(f),
            Error::ValidateError(err) => err.fmt(f),
            Error::ResolveError(err) => err.fmt(f),
            Error::ImportError(err) => err.fmt(f),
            Error::UsageError(err) => err.fmt(f),
            Error::CondCompError(err) => err.fmt(f),
            Error::TomlError(err) => err.fmt(f),
            #[cfg(feature = "eval")]
            Error::EvalError(err, context) => err.fmt(f, context),
            Error::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl From<wgsl_parse::Error> for Error {
    fn from(source: wgsl_parse::Error) -> Self {
        Error::ParseError(source)
    }
}
impl From<ValidateError> for Error {
    fn from(source: ValidateError) -> Self {
        Error::ValidateError(source)
    }
}
impl From<ResolveError> for Error {
    fn from(source: ResolveError) -> Self {
        Error::ResolveError(source)
    }
}
impl From<ImportError> for Error {
    fn from(source: ImportError) -> Self {
        Error::ImportError(source)
    }
}
impl From<UsageError> for Error {
    fn from(source: UsageError) -> Self {
        Error::UsageError(source)
    }
}
impl From<CondCompError> for Error {
    fn from(source: CondCompError) -> Self {
        Error::CondCompError(source)
    }
}
impl From<TomlError> for Error {
    fn from(source: TomlError) -> Self {
        Error::TomlError(source)
    }
}

/// Error diagnostics. Display user-friendly error snippets with `Display`.
///
/// A diagnostic is a wrapper around an error with extra contextual metadata: the source,
/// the declaration name, the span, ...
#[derive(Debug)]
pub struct Diagnostic {
    pub error: Box<Error>,
    pub detail: Box<Detail>,
}

#[derive(Clone, Debug)]
pub struct Detail {
    pub source: Option<String>,
    pub output: Option<String>,
    pub module_path: Option<ModulePath>,
    pub display_name: Option<String>,
    pub declaration: Option<String>,
    pub span: Option<Span>,
}

impl From<wgsl_parse::Error> for Diagnostic {
    fn from(error: wgsl_parse::Error) -> Self {
        let span = error.span;
        let mut res = Self::new(Error::ParseError(error));
        res.detail.span = Some(span);
        res
    }
}

impl From<ValidateError> for Diagnostic {
    fn from(error: ValidateError) -> Self {
        Self::new(error.into())
    }
}

impl From<ResolveError> for Diagnostic {
    fn from(error: ResolveError) -> Self {
        Self::new(error.into())
    }
}

impl From<ImportError> for Diagnostic {
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::ResolveError(e) => Self::from(e),
            _ => Self::new(error.into()),
        }
    }
}

impl From<UsageError> for Diagnostic {
    fn from(error: UsageError) -> Self {
        Self::new(error.into())
    }
}

impl From<CondCompError> for Diagnostic {
    fn from(error: CondCompError) -> Self {
        Self::new(error.into())
    }
}

impl From<TomlError> for Diagnostic {
    fn from(error: TomlError) -> Self {
        Self::new(error.into())
    }
}

impl From<Error> for Diagnostic {
    fn from(error: Error) -> Self {
        Diagnostic::new(error)
    }
}

impl Diagnostic {
    /// Create an empty diagnostic from an error. No metadata is attached.
    pub fn new(error: Error) -> Diagnostic {
        Self {
            error: Box::new(error),
            detail: Box::new(Detail {
                source: None,
                output: None,
                module_path: None,
                display_name: None,
                declaration: None,
                span: None,
            }),
        }
    }
    /// Provide the source code from which the error was emitted.
    /// You should also provide the span with [`Self::with_span`].
    pub fn with_source(mut self, source: String) -> Self {
        if self.detail.source.is_none() {
            self.detail.source = Some(source);
        }
        self
    }
    /// Provide the span (chunk of source code) where the error originated.
    /// You should also provide the source with [`Self::with_source`].
    /// Subsequent calls to this function do not override the span.
    pub fn with_span(mut self, span: Span) -> Self {
        if self.detail.span.is_none() {
            self.detail.span = Some(span);
        }
        self
    }
    /// Provide the declaration in which the error originated.
    pub fn with_declaration(mut self, decl: String) -> Self {
        if self.detail.declaration.is_none() {
            self.detail.declaration = Some(decl);
        }
        self
    }
    /// Provide the output code that was generated, even if an error was emitted.
    pub fn with_output(mut self, out: String) -> Self {
        if self.detail.output.is_none() {
            self.detail.output = Some(out);
        }
        self
    }
    /// Provide the module path in which the error was emitted. The `disp_name` is
    /// usually the file name of the module.
    pub fn with_module_path(mut self, path: ModulePath, disp_name: Option<String>) -> Self {
        if self.detail.module_path.is_none() {
            self.detail.module_path = Some(path);
            self.detail.display_name = disp_name;
        }
        self
    }
    /// Add metadata collected by the evaluation/execution context.
    #[cfg(feature = "eval")]
    pub fn with_ctx(mut self, ctx: &crate::eval::Context) -> Self {
        let (decl, span) = ctx.err_ctx();
        self.detail.declaration = decl.map(|id| id.to_string());
        self.detail.span = span;
        self
    }

    /// Add metadata collected by the sourcemap. If the mangled declaration name was set,
    /// this will automatically add the source, the module path and the declaration name.
    pub fn with_sourcemap(mut self, sourcemap: &impl SourceMap) -> Self {
        if let Some(decl) = &self.detail.declaration
            && let Some(entry) = sourcemap.item(decl)
        {
            self.detail.module_path = Some(entry.path.clone());
            self.detail.declaration = Some(entry.name.to_string());
            self.detail.display_name = sourcemap
                .display_name(&entry.path)
                .map(|name| name.to_string());
            self.detail.source = sourcemap
                .source(&entry.path)
                .map(|s| s.to_string())
                .or(self.detail.source);
        }

        if self.detail.source.is_none() {
            if let Some(path) = &self.detail.module_path {
                self.detail.source = sourcemap.source(path).map(|s| s.to_string());
            } else {
                self.detail.source = sourcemap.default_source().map(|s| s.to_string());
            }
        }

        self
    }

    pub(crate) fn display_origin(&self) -> String {
        match (&self.detail.module_path, &self.detail.display_name) {
            (Some(res), Some(name)) => {
                format!("{res} ({name})")
            }
            (Some(res), None) => res.to_string(),
            (None, Some(name)) => name.to_string(),
            (None, None) => "unknown module".to_string(),
        }
    }

    pub(crate) fn display_short_origin(&self) -> Option<String> {
        self.detail
            .display_name
            .clone()
            .or_else(|| self.detail.module_path.as_ref().map(|res| res.to_string()))
    }
}

impl Diagnostic {
    /// XXX: this function has issues when the main module identifiers are not mangled.
    /// unmangle any mangled identifiers in the error.
    ///
    /// The mangled must be the same used for compiling the WGSL source. It must have
    /// unmangling capabilities. If not, you might want to use a [`crate::sourcemap::SourceMapper`].
    // TODO: this is no longer used, but should
    #[allow(dead_code, reason = "TODO this should be re-added")]
    fn unmangle(
        mut self,
        sourcemap: Option<&impl SourceMap>,
        mangler: Option<&impl Mangler>,
        context: &TyContext,
    ) -> Self {
        fn unmangle_id(
            id: &mut Ident,
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
        ) {
            let path_name = if let Some(sourcemap) = sourcemap {
                sourcemap
                    .item(&id.name())
                    .map(|entry| (entry.path.clone(), entry.name.to_string()))
            } else if let Some(mangler) = mangler {
                mangler.unmangle(&id.name())
            } else {
                None
            };
            if let Some((path, name)) = path_name {
                *id = Ident::new(format!("{path}::{name}"));
            }
        }

        fn unmangle_name(
            mangled: &mut String,
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
        ) {
            let path_name = if let Some(sourcemap) = sourcemap {
                sourcemap
                    .item(mangled)
                    .map(|entry| (entry.path.clone(), entry.name.to_string()))
            } else if let Some(mangler) = mangler {
                mangler.unmangle(mangled)
            } else {
                None
            };
            if let Some((path, name)) = path_name {
                *mangled = format!("{path}::{name}");
            }
        }

        fn unmangle_expr(
            expr: &mut Expression,
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
        ) {
            match expr {
                Expression::Literal(_) => {}
                Expression::Parenthesized(e) => {
                    unmangle_expr(&mut e.expression, sourcemap, mangler)
                }
                Expression::NamedComponent(e) => unmangle_expr(&mut e.base, sourcemap, mangler),
                Expression::Indexing(e) => unmangle_expr(&mut e.base, sourcemap, mangler),
                Expression::Unary(e) => unmangle_expr(&mut e.operand, sourcemap, mangler),
                Expression::Binary(e) => {
                    unmangle_expr(&mut e.left, sourcemap, mangler);
                    unmangle_expr(&mut e.right, sourcemap, mangler);
                }
                Expression::FunctionCall(e) => {
                    unmangle_id(&mut e.ty.ident, sourcemap, mangler);
                    for arg in &mut e.arguments {
                        unmangle_expr(arg, sourcemap, mangler);
                    }
                }
                Expression::TypeOrIdentifier(ty) => unmangle_id(&mut ty.ident, sourcemap, mangler),
            }
        }

        #[cfg(feature = "eval")]
        fn unmangle_ty(
            mangled: &mut wgsl_types::ty::Type,
            _sourcemap: Option<&impl SourceMap>,
            _mangler: Option<&impl Mangler>,
            _context: &TyContext,
        ) {
            use wgsl_types::ty::Type;
            match mangled {
                // TODO unmangle components!
                Type::Struct(_s) => {
                    // TODO: Unmangle structs
                    // unmangle_name(&mut context[*s].name, sourcemap, mangler);
                    // for m in context[*s].members.iter_mut() {
                    //     unmangle_ty(&mut m.ty, sourcemap, mangler, context);
                    // }
                }
                Type::Array(ty, _) => unmangle_ty(&mut *ty, _sourcemap, _mangler, _context),
                Type::Atomic(ty) => unmangle_ty(&mut *ty, _sourcemap, _mangler, _context),
                Type::Ptr(_, ty, _) => unmangle_ty(&mut *ty, _sourcemap, _mangler, _context),
                Type::Ref(_, ty, _) => unmangle_ty(&mut *ty, _sourcemap, _mangler, _context),
                _ => (),
            }
        }

        #[cfg(feature = "eval")]
        fn unmangle_inst(
            mangled: &mut wgsl_types::inst::Instance,
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
            context: &TyContext,
        ) {
            use wgsl_types::inst::Instance;
            match mangled {
                Instance::Struct(inst) => {
                    // TODO: Unmangle structs
                    // unmangle_name(&mut context[inst.ty].name, sourcemap, mangler);
                    for inst in inst.members.iter_mut() {
                        unmangle_inst(inst, sourcemap, mangler, context);
                    }
                }
                Instance::Array(inst) => {
                    for c in inst.iter_mut() {
                        unmangle_inst(c, sourcemap, mangler, context);
                    }
                }
                Instance::Ptr(inst) => {
                    unmangle_ty(&mut inst.ptr.ty, sourcemap, mangler, context);
                }
                Instance::Ref(inst) => {
                    unmangle_ty(&mut inst.ty, sourcemap, mangler, context);
                }
                Instance::Atomic(inst) => {
                    unmangle_inst(inst.inner_mut(), sourcemap, mangler, context);
                }
                Instance::Opaque(ty) => unmangle_ty(ty, sourcemap, mangler, context),
                Instance::Literal(_) | Instance::Vec(_) | Instance::Mat(_) => {}
            }
        }

        if let Some(decl) = &mut self.detail.declaration {
            unmangle_name(decl, sourcemap, mangler);
        }

        match &mut *self.error {
            Error::ParseError(_) => {}
            Error::ValidateError(e) => match e {
                ValidateError::UndefinedSymbol(name)
                | ValidateError::ParamCount(name, _, _)
                | ValidateError::NotCallable(name)
                | ValidateError::Duplicate(name) => unmangle_name(name, sourcemap, mangler),
                ValidateError::Cycle(name1, name2) => {
                    unmangle_name(name1, sourcemap, mangler);
                    unmangle_name(name2, sourcemap, mangler);
                }
            },
            Error::ResolveError(_) => {}
            Error::ImportError(_) => {}
            Error::UsageError(e) => match e {
                UsageError::NotFound(_, _) => todo!(),
                UsageError::Visibility { orig, decl, .. } => {
                    if let Some((_, id)) = orig {
                        unmangle_id(id, sourcemap, mangler);
                    }
                    unmangle_id(&mut decl.1, sourcemap, mangler);
                }
            },
            Error::CondCompError(e) => match e {
                CondCompError::InvalidExpression(expr) => unmangle_expr(expr, sourcemap, mangler),
                CondCompError::InvalidFeatureFlag(_)
                | CondCompError::UnexpectedFeatureFlag(_)
                | CondCompError::NoPrecedingIf
                | CondCompError::DuplicateIf => {}
            },
            Error::TomlError(_) => {}
            // #[cfg(feature = "generics")]
            // Error::GenericsError(_) => {}
            #[cfg(feature = "eval")]
            Error::EvalError(e, _) => match e {
                EvalError::NotScalar(ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::NotConstructible(ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::Type(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::SampledType(ty) => {
                    unmangle_ty(ty, sourcemap, mangler, context);
                }
                EvalError::NotType(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UnknownType(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UnknownStruct(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::NotAccessible(name, _) => unmangle_name(name, sourcemap, mangler),
                EvalError::UnexpectedTemplate(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::View(ty, _) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::RefType(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::WriteRefType(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::Conversion(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::ConvOverflow(_, ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::Component(ty, _) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::Index(ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::NotIndexable(ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::OutOfBounds(_, ty, _) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::Unary(_, ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::Binary(_, ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::CompwiseBinary(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::UnknownFunction(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::NotCallable(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::Signature(sig) => {
                    unmangle_name(&mut sig.name, sourcemap, mangler);
                    for tplt in sig.tplt.iter_mut().flatten() {
                        match tplt {
                            wgsl_types::tplt::TpltParam::Type(ty) => {
                                unmangle_ty(ty, sourcemap, mangler, context)
                            }
                            wgsl_types::tplt::TpltParam::Instance(inst) => {
                                unmangle_inst(inst, sourcemap, mangler, context)
                            }
                            wgsl_types::tplt::TpltParam::Enumerant(_) => {}
                        }
                    }
                    for arg in &mut sig.args {
                        unmangle_ty(arg, sourcemap, mangler, context);
                    }
                }
                EvalError::ParamCount(name, _, _) => unmangle_name(name, sourcemap, mangler),
                EvalError::ParamType(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::ReturnType(ty1, name, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_name(name, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::NoReturn(name, ty) => {
                    unmangle_name(name, sourcemap, mangler);
                    unmangle_ty(ty, sourcemap, mangler, context);
                }
                EvalError::UnexpectedReturn(name, ty) => {
                    unmangle_name(name, sourcemap, mangler);
                    unmangle_ty(ty, sourcemap, mangler, context);
                }
                EvalError::NotConst(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::Void(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::MustUse(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::NotEntrypoint(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UnknownDecl(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UninitConst(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UninitLet(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UninitOverride(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::DuplicateDecl(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::AssignType(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler, context);
                    unmangle_ty(ty2, sourcemap, mangler, context);
                }
                EvalError::IncrType(ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::DecrType(ty) => unmangle_ty(ty, sourcemap, mangler, context),
                EvalError::ConstAssertFailure(expr) => unmangle_expr(expr, sourcemap, mangler),
                EvalError::Todo(_)
                | EvalError::Unreachable
                | EvalError::MissingTemplate(_)
                | EvalError::NotWrite
                | EvalError::NotRead
                | EvalError::NotReadWrite
                | EvalError::PtrHandle
                | EvalError::PtrVecComp
                | EvalError::Swizzle(_)
                | EvalError::NegOverflow
                | EvalError::AddOverflow
                | EvalError::SubOverflow
                | EvalError::MulOverflow
                | EvalError::DivByZero
                | EvalError::RemZeroDiv
                | EvalError::ShlOverflow(_, _)
                | EvalError::ShrOverflow(_, _)
                | EvalError::Builtin(_)
                | EvalError::TemplateArgs(_)
                | EvalError::InvalidEntrypointParam(_)
                | EvalError::MissingBuiltinInput(_, _)
                | EvalError::OutputBuiltin(_)
                | EvalError::InputBuiltin(_)
                | EvalError::MissingUserInput(_, _)
                | EvalError::OverrideInConst
                | EvalError::OverrideInFn
                | EvalError::LetInMod
                | EvalError::ForbiddenInitializer(_)
                | EvalError::UntypedDecl
                | EvalError::ForbiddenDecl(_, _)
                | EvalError::MissingResource(_, _)
                | EvalError::AddressSpace(_, _)
                | EvalError::AccessMode(_, _)
                | EvalError::MissingBindAttr
                | EvalError::MissingWorkgroupSize
                | EvalError::NegativeAttr(_)
                | EvalError::InvalidBlendSrc(_)
                | EvalError::NotRef(_)
                | EvalError::IncrOverflow
                | EvalError::DecrOverflow
                | EvalError::FlowInContinuing(_)
                | EvalError::DiscardInConst
                | EvalError::FlowInFunction(_)
                | EvalError::FlowInModule(_) => {}
            },
            Error::Custom(_) => {}
        };

        self
    }
}

impl Diagnostic {
    fn render_snippet(&self, renderer: &annotate_snippets::Renderer) -> String {
        use annotate_snippets::*;
        let msg = format!("{}", self.error);
        let title = Level::ERROR.primary_title(&msg);
        let mut group = Group::with_title(title);

        let orig = self.display_origin();
        let short_orig = self.display_short_origin();

        if let Some(span) = &self.detail.span {
            let source = self.detail.source.as_deref();

            if let Some(source) = source {
                if span.range().end <= source.len() {
                    let annot = AnnotationKind::Primary.span(span.range()).label(&msg);
                    let mut snip = Snippet::source(source).fold(true).annotation(annot);

                    if let Some(orig) = &short_orig {
                        snip = snip.path(orig);
                    }

                    group = group.element(snip);
                } else {
                    group = group.element(
                        Level::NOTE.message("cannot display snippet: invalid source location"),
                    )
                }
            } else {
                group = group
                    .element(Level::NOTE.message("cannot display snippet: missing source file"))
            }
        }

        let note;
        if let Some(decl) = &self.detail.declaration {
            note = format!("in declaration of `{decl}` in {orig}");
        } else {
            note = format!("in {orig}");
        }
        let group = group.element(Level::NOTE.message(&note));

        renderer.render(&[group])
    }

    pub fn render_plain(&self) -> String {
        let renderer = annotate_snippets::Renderer::plain();
        self.render_snippet(&renderer)
    }

    pub fn render_colored(&self) -> String {
        let renderer = annotate_snippets::Renderer::styled();
        self.render_snippet(&renderer)
    }
}

impl std::error::Error for Diagnostic {}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // AutoStream::choice may return `AlwaysAnsi`, `Always` or `Never`.
        // It never returns `Auto`.
        // It checks terminal capabilities as well as env vars:
        // `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE` and `CI`.
        let choice = anstream::AutoStream::choice(&std::io::stdout());
        if choice == anstream::ColorChoice::Always || choice == anstream::ColorChoice::AlwaysAnsi {
            write!(f, "{}", self.render_colored())
        } else {
            write!(f, "{}", self.render_plain())
        }
    }
}

/// Wrapper type that provides forces a colored/non-colored output.
#[derive(Debug)]
pub struct ColoredDiagnostic<'a>(&'a Diagnostic, bool);

impl Diagnostic {
    /// A [`std::fmt::Display`] adapter to get a colored output (with ANSI codes).
    ///
    /// Diagnostics auto-detect if colors are supported using `anstream`.
    /// With this adapter, you can force a colored or non-colored output.
    pub fn colored(&self, colored: bool) -> ColoredDiagnostic<'_> {
        ColoredDiagnostic(self, colored)
    }
}

impl Error {
    /// Convert this error to a [`Diagnostic`].
    ///
    /// Diagnostics provide better-looking error messages.
    /// Use [`Diagnostic::colored`] to get a colored output similar to Rust's compiler errors.
    pub fn diagnostic(self) -> Diagnostic {
        Diagnostic::new(self)
    }
}

impl std::error::Error for ColoredDiagnostic<'_> {}

impl std::fmt::Display for ColoredDiagnostic<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.1 {
            write!(f, "{}", self.0.render_colored())
        } else {
            write!(f, "{}", self.0.render_plain())
        }
    }
}
