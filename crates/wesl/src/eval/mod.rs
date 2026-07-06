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
pub use wgsl_types::{ShaderStage, builtin::*, conv::*, inst::*, tplt::*, ty::*};

use derive_more::Display;
use std::{collections::HashMap, rc::Rc};
use wgsl_parse::{
    span::{Span, Spanned},
    syntax::*,
};

use crate::util::IteratorExt;

#[derive(Debug)]
struct ScopeInner<T> {
    local: HashMap<String, T>,
    parent: Option<Rc<ScopeInner<T>>>,
    transparent: bool,
}

#[derive(Debug)]
pub struct Scope<T> {
    inner: Rc<ScopeInner<T>>,
}

impl<T> Default for Scope<T> {
    fn default() -> Self {
        Self {
            inner: Rc::new(ScopeInner {
                local: Default::default(),
                parent: Default::default(),
                transparent: false,
            }),
        }
    }
}

impl<T> ScopeInner<T> {
    pub fn depth(&self) -> u32 {
        if let Some(parent) = &self.parent {
            parent.depth() + 1
        } else {
            0
        }
    }
    pub fn get(&self, name: &str) -> Option<&T> {
        self.local
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|parent| parent.get(name)))
    }
    pub fn contains(&self, name: &str) -> bool {
        self.local.contains_key(name)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.contains(name))
    }
    pub fn iter_keys(&self) -> impl Iterator<Item = &str> {
        self.local.keys().map(|k| k.as_str()).chain(
            self.parent
                .iter()
                .flat_map(|parent| parent.iter_keys())
                .boxed(),
        )
    }
}

impl<T> Scope<T> {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(ScopeInner {
                local: Default::default(),
                parent: None,
                transparent: false,
            }),
        }
    }
    pub fn is_root(&self) -> bool {
        self.inner.parent.is_none()
    }
    pub fn depth(&self) -> u32 {
        self.inner.depth()
    }
    /// variables in a 'transparent' scope have the same scope as the parent scope.
    /// this is useful for 'for' loops and function calls which have the same
    /// end-of-scope for initializer and formal parameters as the body.
    /// When the transparent scope ends, its declarations are dropped.
    ///
    /// see <https://github.com/gpuweb/gpuweb/issues/5024>
    pub fn make_transparent(&mut self) {
        Rc::get_mut(&mut self.inner)
            .expect("cannot edit a parent scope")
            .transparent = true;
    }
    pub fn push(&mut self) {
        self.inner = Rc::new(ScopeInner {
            local: Default::default(),
            parent: Some(self.inner.clone()),
            transparent: false,
        });
    }
    pub fn push_transparent(&mut self) {
        self.inner = Rc::new(ScopeInner {
            local: Default::default(),
            parent: Some(self.inner.clone()),
            transparent: true,
        });
    }
    pub fn pop(&mut self) {
        self.inner = self.inner.parent.clone().expect("failed to pop scope");
    }
    pub fn add(&mut self, name: String, value: T) -> bool {
        if self.local_contains(&name) {
            false
        } else {
            Rc::get_mut(&mut self.inner)
                .expect("cannot edit a parent scope")
                .local
                .insert(name, value);
            true
        }
    }
    pub fn local_get_mut(&mut self, name: &str) -> Option<&mut T> {
        Rc::get_mut(&mut self.inner)
            .expect("cannot edit a parent scope")
            .local
            .get_mut(name)
    }
    pub fn get(&self, name: &str) -> Option<&T> {
        self.inner.get(name)
    }
    pub fn local_contains(&self, name: &str) -> bool {
        self.inner.local.contains_key(name)
            || self.inner.transparent
                && self
                    .inner
                    .parent
                    .as_ref()
                    .expect("transparent scope must have a parent")
                    .local
                    .contains_key(name)
    }
    pub fn contains(&self, name: &str) -> bool {
        self.inner.contains(name)
    }
    pub fn iter_keys(&self) -> impl Iterator<Item = &str> {
        self.inner.iter_keys()
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
    pub(crate) kind: ScopeKind,
    pub(crate) stage: ShaderStage,
    pub(crate) err_decl: Option<String>,
    pub(crate) err_span: Option<Span>,
}

impl<'s> Context<'s> {
    pub fn new(source: &'s TranslationUnit) -> Self {
        Self {
            source,
            scope: Default::default(),
            resources: Default::default(),
            overrides: Default::default(),
            kind: ScopeKind::Function,
            stage: ShaderStage::Const,
            err_span: None,
            err_decl: None,
        }
    }

    pub fn source(&self) -> &TranslationUnit {
        self.source
    }

    fn set_err_decl_ctx(&mut self, decl: String) {
        if self.err_decl.is_none() {
            self.err_decl = Some(decl)
        }
    }
    fn set_err_span_ctx(&mut self, expr: Span) {
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
}

pub trait SyntaxUtil {
    /// find a global declaration by name.
    fn user_decl(&self, name: &str) -> Option<&GlobalDeclaration>;

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
