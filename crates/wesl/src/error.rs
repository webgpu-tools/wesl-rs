use std::fmt::Display;

use wgsl_parse::{
    span::Span,
    syntax::{Expression, Ident, ModulePath},
};

use crate::{Mangler, ResolveError, SourceMap, ValidateError};

#[cfg(feature = "generics")]
use crate::GenericsError;

use crate::CondCompError;
use crate::ImportError;

#[cfg(feature = "eval")]
use crate::eval::{Context, EvalError};

/// Any WESL error.
#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    ParseError(#[from] wgsl_parse::Error),
    #[error("{0}")]
    ValidateError(#[from] ValidateError),
    #[error("{0}")]
    ResolveError(#[from] ResolveError),
    #[error("{0}")]
    ImportError(#[from] ImportError),
    #[error("{0}")]
    CondCompError(#[from] CondCompError),
    #[cfg(feature = "generics")]
    #[error("{0}")]
    GenericsError(#[from] GenericsError),
    #[cfg(feature = "eval")]
    #[error("{0}")]
    EvalError(#[from] EvalError),
    #[error("{0}")]
    Error(#[from] Diagnostic<Error>),
    #[error("{0}")]
    Custom(String),
}

/// Error diagnostics. Display user-friendly error snippets with `Display`.
///
/// A diagnostic is a wrapper around an error with extra contextual metadata: the source,
/// the declaration name, the span, ...
#[derive(Clone, Debug)]
pub struct Diagnostic<E: std::error::Error> {
    pub error: Box<E>,
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

impl From<wgsl_parse::Error> for Diagnostic<Error> {
    fn from(error: wgsl_parse::Error) -> Self {
        let span = error.span;
        let mut res = Self::new(Error::ParseError(error));
        res.detail.span = Some(span);
        res
    }
}

impl From<ValidateError> for Diagnostic<Error> {
    fn from(error: ValidateError) -> Self {
        Self::new(error.into())
    }
}

impl From<ResolveError> for Diagnostic<Error> {
    fn from(error: ResolveError) -> Self {
        match error {
            ResolveError::FileNotFound(_, _) | ResolveError::ModuleNotFound(_, _) => {
                Self::new(error.into())
            }
            ResolveError::Error(e) => e,
        }
    }
}

impl From<ImportError> for Diagnostic<Error> {
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::ResolveError(e) => Self::from(e),
            _ => Self::new(error.into()),
        }
    }
}

impl From<CondCompError> for Diagnostic<Error> {
    fn from(error: CondCompError) -> Self {
        Self::new(error.into())
    }
}

#[cfg(feature = "generics")]
impl From<GenericsError> for Diagnostic<Error> {
    fn from(error: GenericsError) -> Self {
        Self::new(error.into())
    }
}

#[cfg(feature = "eval")]
impl From<EvalError> for Diagnostic<Error> {
    fn from(error: EvalError) -> Self {
        Self::new(error.into())
    }
}

impl From<Error> for Diagnostic<Error> {
    fn from(error: Error) -> Self {
        match error {
            Error::ParseError(e) => e.into(),
            Error::ResolveError(e) => e.into(),
            Error::ImportError(e) => e.into(),
            Error::Error(e) => e,
            error => Self::new(error),
        }
    }
}

impl<E: std::error::Error> Diagnostic<E> {
    /// Create an empty diagnostic from an error. No metadata is attached.
    fn new(error: E) -> Diagnostic<E> {
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
    pub fn with_ctx(mut self, ctx: &Context) -> Self {
        let (decl, span) = ctx.err_ctx();
        self.detail.declaration = decl.map(|id| id.to_string());
        self.detail.span = span;
        self
    }

    /// Add metadata collected by the sourcemap. If the mangled declaration name was set,
    /// this will automatically add the source, the module path and the declaration name.
    pub fn with_sourcemap(mut self, sourcemap: &impl SourceMap) -> Self {
        if let Some(decl) = &self.detail.declaration
            && let Some((path, decl)) = sourcemap.get_decl(decl)
        {
            self.detail.module_path = Some(path.clone());
            self.detail.declaration = Some(decl.to_string());
            self.detail.display_name = sourcemap
                .get_display_name(path)
                .map(|name| name.to_string());
            self.detail.source = sourcemap
                .get_source(path)
                .map(|s| s.to_string())
                .or(self.detail.source);
        }

        if self.detail.source.is_none() {
            if let Some(path) = &self.detail.module_path {
                self.detail.source = sourcemap.get_source(path).map(|s| s.to_string());
            } else {
                self.detail.source = sourcemap.get_default_source().map(|s| s.to_string());
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

impl Diagnostic<Error> {
    // XXX: this function has issues when the root module identifiers are not mangled.
    /// unmangle any mangled identifiers in the error.
    ///
    /// The mangled must be the same used for compiling the WGSL source. It must have
    /// unmangling capabilities. If not, you might want to use a [`crate::SourceMapper`].
    pub fn unmangle(
        mut self,
        sourcemap: Option<&impl SourceMap>,
        mangler: Option<&impl Mangler>,
    ) -> Self {
        fn unmangle_id(
            id: &mut Ident,
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
        ) {
            let res_name = if let Some(sourcemap) = sourcemap {
                sourcemap
                    .get_decl(&id.name())
                    .map(|(res, name)| (res.clone(), name.to_string()))
            } else if let Some(mangler) = mangler {
                mangler.unmangle(&id.name())
            } else {
                None
            };
            if let Some((res, name)) = res_name {
                *id = Ident::new(format!("{res}::{name}"));
            }
        }

        fn unmangle_name(
            mangled: &mut String,
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
        ) {
            let res_name = if let Some(sourcemap) = sourcemap {
                sourcemap
                    .get_decl(mangled)
                    .map(|(res, name)| (res.clone(), name.to_string()))
            } else if let Some(mangler) = mangler {
                mangler.unmangle(mangled)
            } else {
                None
            };
            if let Some((res, name)) = res_name {
                *mangled = format!("{res}::{name}");
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
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
        ) {
            use wgsl_types::ty::Type;
            match mangled {
                // TODO unmangle components!
                Type::Struct(s) => {
                    unmangle_name(&mut s.name, sourcemap, mangler);
                    for m in s.members.iter_mut() {
                        unmangle_ty(&mut m.ty, sourcemap, mangler);
                    }
                }
                Type::Array(ty, _) => unmangle_ty(&mut *ty, sourcemap, mangler),
                Type::Atomic(ty) => unmangle_ty(&mut *ty, sourcemap, mangler),
                Type::Ptr(_, ty, _) => unmangle_ty(&mut *ty, sourcemap, mangler),
                Type::Ref(_, ty, _) => unmangle_ty(&mut *ty, sourcemap, mangler),
                _ => (),
            }
        }

        #[cfg(feature = "eval")]
        fn unmangle_inst(
            mangled: &mut wgsl_types::inst::Instance,
            sourcemap: Option<&impl SourceMap>,
            mangler: Option<&impl Mangler>,
        ) {
            use wgsl_types::inst::Instance;
            match mangled {
                Instance::Struct(inst) => {
                    unmangle_name(&mut inst.ty.name, sourcemap, mangler);
                    for inst in inst.members.iter_mut() {
                        unmangle_inst(inst, sourcemap, mangler);
                    }
                }
                Instance::Array(inst) => {
                    for c in inst.iter_mut() {
                        unmangle_inst(c, sourcemap, mangler);
                    }
                }
                Instance::Ptr(inst) => {
                    unmangle_ty(&mut inst.ptr.ty, sourcemap, mangler);
                }
                Instance::Ref(inst) => {
                    unmangle_ty(&mut inst.ty, sourcemap, mangler);
                }
                Instance::Atomic(inst) => {
                    unmangle_inst(inst.inner_mut(), sourcemap, mangler);
                }
                Instance::Opaque(ty) => unmangle_ty(ty, sourcemap, mangler),
                Instance::Literal(_) | Instance::Vec(_) | Instance::Mat(_) => {}
            }
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
            Error::CondCompError(e) => match e {
                CondCompError::InvalidExpression(expr) => unmangle_expr(expr, sourcemap, mangler),
                CondCompError::InvalidFeatureFlag(_)
                | CondCompError::UnexpectedFeatureFlag(_)
                | CondCompError::NoPrecedingIf
                | CondCompError::DuplicateIf => {}
            },
            #[cfg(feature = "generics")]
            Error::GenericsError(_) => {}
            #[cfg(feature = "eval")]
            Error::EvalError(e) => match e {
                EvalError::NotScalar(ty) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::NotConstructible(ty) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::Type(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::SampledType(ty) => {
                    unmangle_ty(ty, sourcemap, mangler);
                }
                EvalError::NotType(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UnknownType(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::UnknownStruct(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::NotAccessible(name, _) => unmangle_name(name, sourcemap, mangler),
                EvalError::UnexpectedTemplate(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::View(ty, _) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::RefType(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::WriteRefType(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::Conversion(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::ConvOverflow(_, ty) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::Component(ty, _) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::Index(ty) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::NotIndexable(ty) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::OutOfBounds(_, ty, _) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::Unary(_, ty) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::Binary(_, ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::CompwiseBinary(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::UnknownFunction(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::NotCallable(name) => unmangle_name(name, sourcemap, mangler),
                EvalError::Signature(sig) => {
                    unmangle_name(&mut sig.name, sourcemap, mangler);
                    for tplt in sig.tplt.iter_mut().flatten() {
                        match tplt {
                            wgsl_types::tplt::TpltParam::Type(ty) => {
                                unmangle_ty(ty, sourcemap, mangler)
                            }
                            wgsl_types::tplt::TpltParam::Instance(inst) => {
                                unmangle_inst(inst, sourcemap, mangler)
                            }
                            wgsl_types::tplt::TpltParam::Enumerant(_) => {}
                        }
                    }
                    for arg in &mut sig.args {
                        unmangle_ty(arg, sourcemap, mangler);
                    }
                }
                EvalError::ParamCount(name, _, _) => unmangle_name(name, sourcemap, mangler),
                EvalError::ParamType(ty1, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::ReturnType(ty1, name, ty2) => {
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_name(name, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::NoReturn(name, ty) => {
                    unmangle_name(name, sourcemap, mangler);
                    unmangle_ty(ty, sourcemap, mangler);
                }
                EvalError::UnexpectedReturn(name, ty) => {
                    unmangle_name(name, sourcemap, mangler);
                    unmangle_ty(ty, sourcemap, mangler);
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
                    unmangle_ty(ty1, sourcemap, mangler);
                    unmangle_ty(ty2, sourcemap, mangler);
                }
                EvalError::IncrType(ty) => unmangle_ty(ty, sourcemap, mangler),
                EvalError::DecrType(ty) => unmangle_ty(ty, sourcemap, mangler),
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
            Error::Error(_) => {}
            Error::Custom(_) => {}
        };

        self
    }
}

impl<E: std::error::Error> std::error::Error for Diagnostic<E> {}

impl<E: std::error::Error> Display for Diagnostic<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

        let renderer = Renderer::styled();
        let rendered = renderer.render(&[group]);
        write!(f, "{rendered}")
    }
}
