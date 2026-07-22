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
        use std::cmp::Ordering;
        type Decl = GlobalDeclaration;
        self.global_declarations
            .sort_unstable_by(|a, b| match (a.node(), b.node()) {
                (Decl::Void, Decl::Void) => Ordering::Equal,
                (Decl::Void, Decl::Declaration(_)) => Ordering::Less,
                (Decl::Void, Decl::Struct(_)) => Ordering::Less,
                (Decl::Void, Decl::TypeAlias(_)) => Ordering::Less,
                (Decl::Void, Decl::ConstAssert(_)) => Ordering::Less,
                (Decl::Void, Decl::Function(_)) => Ordering::Less,

                (Decl::Declaration(_), Decl::Void) => Ordering::Greater,
                (Decl::Declaration(d1), Decl::Declaration(d2)) => {
                    d1.ident.name().cmp(&d2.ident.name())
                }
                (Decl::Declaration(_), Decl::Struct(_)) => Ordering::Less,
                (Decl::Declaration(_), Decl::TypeAlias(_)) => Ordering::Less,
                (Decl::Declaration(_), Decl::ConstAssert(_)) => Ordering::Less,
                (Decl::Declaration(_), Decl::Function(_)) => Ordering::Less,

                (Decl::Struct(_), Decl::Void) => Ordering::Greater,
                (Decl::Struct(_), Decl::Declaration(_)) => Ordering::Greater,
                (Decl::Struct(d1), Decl::Struct(d2)) => d1.ident.name().cmp(&d2.ident.name()),
                (Decl::Struct(_), Decl::TypeAlias(_)) => Ordering::Less,
                (Decl::Struct(_), Decl::ConstAssert(_)) => Ordering::Less,
                (Decl::Struct(_), Decl::Function(_)) => Ordering::Less,

                (Decl::TypeAlias(_), Decl::Void) => Ordering::Greater,
                (Decl::TypeAlias(_), Decl::Declaration(_)) => Ordering::Greater,
                (Decl::TypeAlias(_), Decl::Struct(_)) => Ordering::Greater,
                (Decl::TypeAlias(d1), Decl::TypeAlias(d2)) => d1.ident.name().cmp(&d2.ident.name()),
                (Decl::TypeAlias(_), Decl::ConstAssert(_)) => Ordering::Less,
                (Decl::TypeAlias(_), Decl::Function(_)) => Ordering::Less,

                (Decl::ConstAssert(_), Decl::Void) => Ordering::Greater,
                (Decl::ConstAssert(_), Decl::Declaration(_)) => Ordering::Greater,
                (Decl::ConstAssert(_), Decl::Struct(_)) => Ordering::Greater,
                (Decl::ConstAssert(_), Decl::TypeAlias(_)) => Ordering::Greater,
                (Decl::ConstAssert(_), Decl::ConstAssert(_)) => Ordering::Equal,
                (Decl::ConstAssert(_), Decl::Function(_)) => Ordering::Less,

                (Decl::Function(_), Decl::Void) => Ordering::Greater,
                (Decl::Function(_), Decl::Declaration(_)) => Ordering::Greater,
                (Decl::Function(_), Decl::Struct(_)) => Ordering::Greater,
                (Decl::Function(_), Decl::TypeAlias(_)) => Ordering::Greater,
                (Decl::Function(_), Decl::ConstAssert(_)) => Ordering::Greater,
                (Decl::Function(d1), Decl::Function(d2)) => d1.ident.name().cmp(&d2.ident.name()),
            });
    }
}
