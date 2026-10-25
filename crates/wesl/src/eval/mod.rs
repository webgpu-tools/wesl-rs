//! Typechecking, Evaluation and Validation.

mod attrs;
mod constant;
mod error;
#[allow(clippy::module_inception)]
mod eval;
mod exec;
mod lower;
mod prelude;
mod to_expr;
mod ty;

pub use attrs::*;
pub(crate) use constant::*;
pub use error::*;
pub use eval::*;
pub use exec::*;
pub use lower::*;
pub use prelude::*;
pub use to_expr::*;
pub use ty::*;
use wgsl_types::arena::{Arena, Id};
pub use wgsl_types::{ShaderStage, builtin::*, conv::*, inst::*, tplt::*, ty::*, ty_ctx::*};

use derive_more::Display;
use std::collections::HashMap;
use wgsl_parse::{
    span::{Span, Spanned},
    syntax::*,
};

pub type ScopeId<T> = Id<ScopeData<T>>;

#[derive(Debug)]
pub struct Scope<T> {
    scopes: Arena<ScopeData<T>>,
    inner: ScopeId<T>,
}

#[derive(Debug)]
pub struct ScopeData<T> {
    parent: Option<ScopeId<T>>,
    local: HashMap<String, T>,
    transparent: bool,
}

impl<T> Default for Scope<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Scope<T> {
    pub fn new() -> Self {
        let mut scopes = Arena::default();
        let inner = scopes.add(ScopeData {
            local: Default::default(),
            parent: Default::default(),
            transparent: false,
        });
        Self { scopes, inner }
    }
    pub fn is_root(&self) -> bool {
        self.scopes[self.inner].parent.is_none()
    }

    /// variables in a 'transparent' scope have the same scope as the parent scope.
    /// this is useful for 'for' loops and function calls which have the same
    /// end-of-scope for initializer and formal parameters as the body.
    /// When the transparent scope ends, its declarations are dropped.
    ///
    /// see <https://github.com/gpuweb/gpuweb/issues/5024>
    pub fn make_transparent(&mut self) {
        self.scopes[self.inner].transparent = true;
    }
    pub fn push(&mut self) {
        self.inner = self.scopes.add(ScopeData {
            local: Default::default(),
            parent: Some(self.inner),
            transparent: false,
        });
    }
    pub fn push_transparent(&mut self) {
        self.inner = self.scopes.add(ScopeData {
            local: Default::default(),
            parent: Some(self.inner),
            transparent: true,
        });
    }
    pub fn pop(&mut self) {
        self.inner = self.scopes[self.inner].parent.expect("failed to pop scope");
    }
    pub fn add(&mut self, name: String, value: T) -> bool {
        if self.local_contains(&name) {
            false
        } else {
            self.scopes[self.inner].local.insert(name, value);
            true
        }
    }
    pub fn local_get_mut(&mut self, name: &str) -> Option<&mut T> {
        self.scopes[self.inner].local.get_mut(name)
    }
    pub fn get(&self, name: &str) -> Option<&T> {
        self.scope_chain()
            .find_map(|scope| self.scopes[scope].local.get(name))
    }
    pub fn local_contains(&self, name: &str) -> bool {
        let inner = &self.scopes[self.inner];
        inner.local.contains_key(name)
            || inner.transparent
                && self.scopes[inner.parent.expect("transparent scope must have a parent")]
                    .local
                    .contains_key(name)
    }
    pub fn contains(&self, name: &str) -> bool {
        self.scope_chain()
            .any(|scope| self.scopes[scope].local.contains_key(name))
    }
    pub fn scope_chain(&self) -> impl Iterator<Item = ScopeId<T>> + '_ {
        std::iter::successors(Some(self.inner), move |&scope| self.scopes[scope].parent)
    }
    pub fn iter_keys(&self) -> impl Iterator<Item = &str> {
        self.scope_chain()
            .flat_map(move |scope| self.scopes[scope].local.keys().map(|k| k.as_str()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Display)]
pub enum ScopeKind {
    #[display("module")]
    Module,
    #[display("function")]
    Function,
}

#[derive(Clone, Copy, Debug)]
pub enum ResourceKind {
    UniformBuffer,
    StorageBuffer,
    Texture,
    Sampler,
}

// TODO: should we remove the source from the Context struct?
pub struct Context<'s> {
    pub(crate) source: &'s TranslationUnit,
    // the instance is None if not accessible in the ShaderStage
    pub(crate) scope: Scope<Instance>,
    pub(crate) resources: HashMap<(u32, u32), RefInstance>,
    pub(crate) overrides: HashMap<String, Instance>,
    pub(crate) ty_ctx: &'s mut TyContext,
    pub(crate) kind: ScopeKind,
    pub(crate) stage: ShaderStage,
    pub(crate) err_decl: Option<String>,
    pub(crate) err_span: Option<Span>,
}

impl<'s> Context<'s> {
    pub fn new(source: &'s TranslationUnit, ty_ctx: &'s mut TyContext) -> Self {
        Self {
            source,
            scope: Default::default(),
            resources: Default::default(),
            overrides: Default::default(),
            kind: ScopeKind::Function,
            stage: ShaderStage::Const,
            ty_ctx,
            err_span: None,
            err_decl: None,
        }
    }

    pub fn source(&self) -> &TranslationUnit {
        self.source
    }

    pub(crate) fn set_err_decl_ctx(&mut self, decl: String) {
        if self.err_decl.is_none() {
            self.err_decl = Some(decl)
        }
    }
    pub(crate) fn set_err_span_ctx(&mut self, expr: Span) {
        if self.err_span.is_none() {
            self.err_span = Some(expr)
        }
    }

    pub fn err_ctx(&self) -> (Option<String>, Option<Span>) {
        (self.err_decl.clone(), self.err_span)
    }

    pub fn set_stage(&mut self, stage: ShaderStage) {
        self.stage = stage;
    }

    pub fn add_bindings(&mut self, resources: impl IntoIterator<Item = ((u32, u32), RefInstance)>) {
        for ((group, binding), inst) in resources.into_iter() {
            self.add_binding(group, binding, inst);
        }
    }
    pub fn add_binding(&mut self, group: u32, binding: u32, inst: RefInstance) {
        self.resources.insert((group, binding), inst);
    }
    pub fn resource(&self, group: u32, binding: u32) -> Option<&RefInstance> {
        self.resources.get(&(group, binding))
    }
    pub fn add_overrides(&mut self, overrides: impl IntoIterator<Item = (String, Instance)>) {
        self.overrides.extend(overrides);
    }
    pub fn add_overridable(&mut self, name: String, inst: Instance) {
        self.overrides.insert(name, inst);
    }
    pub fn overridable(&self, name: &str) -> Option<&Instance> {
        self.overrides.get(name)
    }
    pub fn ty_ctx(&self) -> &TyContext {
        self.ty_ctx
    }
}

pub trait SyntaxUtil {
    /// find a global declaration by name.
    fn user_decl(&self, name: &str) -> Option<&GlobalDeclaration>;

    /// find the span of a global declaration by name.
    fn user_decl_span(&self, name: &str) -> Option<Span>;

    /// find a global declaration by name, including built-in ones.
    fn decl(&self, name: &str) -> Option<&GlobalDeclaration>;

    /// find a variable/value declaration by name.
    fn decl_decl(&self, name: &str) -> Option<&Declaration>;

    /// find a type alias declaration by name.
    fn decl_alias(&self, name: &str) -> Option<&TypeAlias>;

    /// find a struct declaration by name.
    ///
    /// see also: [`Self::resolve_alias`] to resolve the name before calling this function.
    fn decl_struct(&self, name: &str) -> Option<&Struct>;

    /// find a function declaration by name.
    fn decl_function(&self, name: &str) -> Option<&Function>;

    /// resolve an alias name.
    fn resolve_alias(&self, name: &str) -> Option<&TypeExpression>;

    /// resolve an aliases in a type expression.
    fn resolve_ty<'a>(&'a self, ty: &'a TypeExpression) -> &'a TypeExpression;
}

impl SyntaxUtil for TranslationUnit {
    fn user_decl(&self, name: &str) -> Option<&GlobalDeclaration> {
        // note: declarations in PRELUDE can be shadowed by user-defined declarations.
        self.global_declarations
            .iter()
            .map(Spanned::node)
            .find(|d| match d {
                GlobalDeclaration::Declaration(d) => *d.ident.name() == name,
                GlobalDeclaration::TypeAlias(d) => *d.ident.name() == name,
                GlobalDeclaration::Struct(d) => *d.ident.name() == name,
                GlobalDeclaration::Function(d) => *d.ident.name() == name,
                _ => false,
            })
    }
    fn user_decl_span(&self, name: &str) -> Option<Span> {
        self.global_declarations
            .iter()
            .find(|d| match d.node() {
                GlobalDeclaration::Declaration(d) => *d.ident.name() == name,
                GlobalDeclaration::TypeAlias(d) => *d.ident.name() == name,
                GlobalDeclaration::Struct(d) => *d.ident.name() == name,
                GlobalDeclaration::Function(d) => *d.ident.name() == name,
                _ => false,
            })
            .map(|d| d.span())
    }
    fn decl(&self, name: &str) -> Option<&GlobalDeclaration> {
        // note: declarations in PRELUDE can be shadowed by user-defined declarations.
        self.global_declarations
            .iter()
            .chain(PRELUDE.global_declarations.iter())
            .map(Spanned::node)
            .find(|d| match d {
                GlobalDeclaration::Declaration(d) => *d.ident.name() == name,
                GlobalDeclaration::TypeAlias(d) => *d.ident.name() == name,
                GlobalDeclaration::Struct(d) => *d.ident.name() == name,
                GlobalDeclaration::Function(d) => *d.ident.name() == name,
                _ => false,
            })
    }
    fn decl_decl(&self, name: &str) -> Option<&Declaration> {
        match self.decl(name) {
            Some(GlobalDeclaration::Declaration(s)) => Some(s),
            _ => None,
        }
    }
    fn decl_alias(&self, name: &str) -> Option<&TypeAlias> {
        match self.decl(name) {
            Some(GlobalDeclaration::TypeAlias(s)) => Some(s),
            _ => None,
        }
    }
    fn decl_struct(&self, name: &str) -> Option<&Struct> {
        match self.decl(name) {
            Some(GlobalDeclaration::Struct(s)) => Some(s),
            _ => None,
        }
    }
    fn decl_function(&self, name: &str) -> Option<&Function> {
        match self.decl(name) {
            Some(GlobalDeclaration::Function(f)) => Some(f),
            _ => None,
        }
    }

    fn resolve_alias(&self, name: &str) -> Option<&TypeExpression> {
        if let Some(alias) = self.decl_alias(name) {
            if alias.ty.template_args.is_none() {
                self.resolve_alias(&alias.ty.ident.name())
                    .or(Some(&alias.ty))
            } else {
                Some(&alias.ty)
            }
        } else {
            None
        }
    }

    fn resolve_ty<'a>(&'a self, ty: &'a TypeExpression) -> &'a TypeExpression {
        self.resolve_alias(&ty.ident.name()).unwrap_or(ty)
    }
}
