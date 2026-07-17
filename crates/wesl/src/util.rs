//! [`SyntaxUtil`] is an extension trait for [`TranslationUnit`].

use std::{collections::HashMap, iter::Iterator};

use itertools::Itertools;
use wgsl_parse::syntax::*;

/// was that not in the std at some point???
type BoxedIterator<'a, T> = Box<dyn Iterator<Item = T> + 'a>;

#[allow(dead_code)]
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

#[derive(Clone, Debug)]
pub struct ImportedItem {
    pub path: ModulePath,
    pub ident: Ident, // this is the ident's original name before `as` renaming.
    pub public: bool,
}

pub type Imports = HashMap<Ident, ImportedItem>;

pub trait SyntaxUtil {
    fn entry_points(&self) -> impl Iterator<Item = &Ident>;
    fn flatten_imports(&self, path: &ModulePath) -> Imports;
}

impl SyntaxUtil for TranslationUnit {
    fn entry_points(&self) -> impl Iterator<Item = &Ident> {
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
                    .then_some(&decl.ident),
                _ => None,
            })
    }

    fn flatten_imports(&self, path: &ModulePath) -> Imports {
        fn rec(content: &ImportContent, path: ModulePath, public: bool, res: &mut Imports) {
            match content {
                ImportContent::Item(item) => {
                    let ident = item.rename.as_ref().unwrap_or(&item.ident).clone();
                    res.insert(
                        ident,
                        ImportedItem {
                            path,
                            ident: item.ident.clone(),
                            public,
                        },
                    );
                }
                ImportContent::Collection(coll) => {
                    for import in coll {
                        let path = path.clone().join(import.path.iter().cloned());
                        rec(&import.content, path, public, res);
                    }
                }
            }
        }

        let mut res = Imports::default();

        for import in &self.imports {
            let public = import.attributes.iter().any(|attr| attr.is_publish());
            match &import.path {
                Some(import_path) => {
                    let path = path.join_path(import_path);
                    rec(&import.content, path, public, &mut res);
                }
                None => {
                    // this covers two cases: `import foo;` and `import {foo, ..};`.
                    // COMBAK: these edge-cases smell
                    match &import.content {
                        ImportContent::Item(_) => {
                            // `import foo`, this import statement does nothing currently.
                            // In the future, it may become a visibility/re-export mechanism.
                        }
                        ImportContent::Collection(coll) => {
                            for import in coll {
                                let mut components = import.path.iter().cloned();
                                if let Some(pkg_name) = components.next() {
                                    // `import {foo::bar}`, foo becomes the package name.
                                    let path = ModulePath::new(
                                        PathOrigin::Package(pkg_name),
                                        components.collect_vec(),
                                    );
                                    rec(&import.content, path, public, &mut res);
                                }
                            }
                        }
                    }
                }
            }
        }

        res
    }
}
