#![allow(dead_code, reason = "private utility module")]
//! [`SyntaxUtil`] is an extension trait for [`TranslationUnit`].

use std::iter::Iterator;

use wgsl_parse::{SyntaxNode, syntax::*};

/// was that not in the std at some point???
type BoxedIterator<'a, T> = Box<dyn Iterator<Item = T> + 'a>;

pub trait IteratorExt: Iterator {
    fn boxed<'a>(self) -> BoxedIterator<'a, Self::Item>
    where
        Self: Sized + 'a;
}

impl<T: Iterator> IteratorExt for T {
    fn boxed<'a>(self) -> BoxedIterator<'a, Self::Item>
    where
        Self: Sized + 'a,
    {
        Box::new(self)
    }
}

pub trait SyntaxUtil {
    fn decl_ident(&self, name: &str) -> Option<Ident>;
    fn entry_points(&self) -> impl Iterator<Item = Ident>;
    fn sort_decls(&mut self);
}

impl SyntaxUtil for TranslationUnit {
    fn decl_ident(&self, name: &str) -> Option<Ident> {
        self.global_declarations
            .iter()
            .find_map(|decl| match decl.ident() {
                Some(ident) if &**ident.name() == name => Some(ident),
                _ => None,
            })
    }

    fn entry_points(&self) -> impl Iterator<Item = Ident> {
        self.global_declarations
            .iter()
            .filter_map(|decl| match decl.node() {
                GlobalDeclaration::Function(decl) => decl
                    .attributes
                    .iter()
                    .any(|attr| {
                        matches!(
                            attr.node(),
                            Attribute::Vertex | Attribute::Fragment | Attribute::Compute
                        )
                    })
                    .then_some(decl.ident.clone()),
                _ => None,
            })
    }

    fn sort_decls(&mut self) {
        use std::cmp::Ordering::*;
        type Decl = GlobalDeclaration;
        self.global_declarations
            .sort_unstable_by(|a, b| match (a.node(), b.node()) {
                (Decl::Void, Decl::Void) => Equal,
                (Decl::Void, Decl::Declaration(_)) => Less,
                (Decl::Void, Decl::Struct(_)) => Less,
                (Decl::Void, Decl::TypeAlias(_)) => Less,
                (Decl::Void, Decl::ConstAssert(_)) => Less,
                (Decl::Void, Decl::Function(_)) => Less,

                (Decl::Declaration(_), Decl::Void) => Greater,
                (Decl::Declaration(d1), Decl::Declaration(d2)) => {
                    // sort in this order: const < override < let < var
                    // then sort by name.
                    match (d1.kind, d2.kind) {
                        (DeclarationKind::Const, DeclarationKind::Const)
                        | (DeclarationKind::Override, DeclarationKind::Override)
                        | (DeclarationKind::Let, DeclarationKind::Let)
                        | (DeclarationKind::Var(_), DeclarationKind::Var(_)) => {
                            d1.ident.name().cmp(&d2.ident.name())
                        }
                        (DeclarationKind::Const, DeclarationKind::Override) => Less,
                        (DeclarationKind::Const, DeclarationKind::Let) => Less,
                        (DeclarationKind::Const, DeclarationKind::Var(_)) => Less,
                        (DeclarationKind::Override, DeclarationKind::Const) => Greater,
                        (DeclarationKind::Override, DeclarationKind::Let) => Less,
                        (DeclarationKind::Override, DeclarationKind::Var(_)) => Less,
                        (DeclarationKind::Let, DeclarationKind::Const) => Greater,
                        (DeclarationKind::Let, DeclarationKind::Override) => Greater,
                        (DeclarationKind::Let, DeclarationKind::Var(_)) => Less,
                        (DeclarationKind::Var(_), DeclarationKind::Const) => Greater,
                        (DeclarationKind::Var(_), DeclarationKind::Override) => Greater,
                        (DeclarationKind::Var(_), DeclarationKind::Let) => Greater,
                    }
                }
                (Decl::Declaration(_), Decl::Struct(_)) => Less,
                (Decl::Declaration(_), Decl::TypeAlias(_)) => Less,
                (Decl::Declaration(_), Decl::ConstAssert(_)) => Less,
                (Decl::Declaration(_), Decl::Function(_)) => Less,

                (Decl::Struct(_), Decl::Void) => Greater,
                (Decl::Struct(_), Decl::Declaration(_)) => Greater,
                (Decl::Struct(d1), Decl::Struct(d2)) => d1.ident.name().cmp(&d2.ident.name()),
                (Decl::Struct(_), Decl::TypeAlias(_)) => Less,
                (Decl::Struct(_), Decl::ConstAssert(_)) => Less,
                (Decl::Struct(_), Decl::Function(_)) => Less,

                (Decl::TypeAlias(_), Decl::Void) => Greater,
                (Decl::TypeAlias(_), Decl::Declaration(_)) => Greater,
                (Decl::TypeAlias(_), Decl::Struct(_)) => Greater,
                (Decl::TypeAlias(d1), Decl::TypeAlias(d2)) => d1.ident.name().cmp(&d2.ident.name()),
                (Decl::TypeAlias(_), Decl::ConstAssert(_)) => Less,
                (Decl::TypeAlias(_), Decl::Function(_)) => Less,

                (Decl::ConstAssert(_), Decl::Void) => Greater,
                (Decl::ConstAssert(_), Decl::Declaration(_)) => Greater,
                (Decl::ConstAssert(_), Decl::Struct(_)) => Greater,
                (Decl::ConstAssert(_), Decl::TypeAlias(_)) => Greater,
                (Decl::ConstAssert(c1), Decl::ConstAssert(c2)) => {
                    // const_assert have no identifiers, we compare the stringification
                    c1.to_string().cmp(&c2.to_string())
                }
                (Decl::ConstAssert(_), Decl::Function(_)) => Less,

                (Decl::Function(_), Decl::Void) => Greater,
                (Decl::Function(_), Decl::Declaration(_)) => Greater,
                (Decl::Function(_), Decl::Struct(_)) => Greater,
                (Decl::Function(_), Decl::TypeAlias(_)) => Greater,
                (Decl::Function(_), Decl::ConstAssert(_)) => Greater,
                (Decl::Function(d1), Decl::Function(d2)) => d1.ident.name().cmp(&d2.ident.name()),
            });
    }
}
