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
}
