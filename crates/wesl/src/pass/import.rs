use itertools::Itertools;
use std::collections::HashMap;
use wgsl_parse::syntax::*;

#[derive(Clone, Debug)]
pub struct ImportedItem {
    pub path: ModulePath,
    pub ident: Ident, // this is the ident's original name before `as` renaming.
    pub public: bool,
}

pub type Imports = HashMap<Ident, ImportedItem>;

/// Flatten imports to a list.
pub fn flatten_imports(imports: &[ImportStatement], path: &ModulePath) -> Imports {
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

    for import in imports {
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

/// Find the normalized module path for an identifier in source, if it refers to an external declaration.
///
/// Inline imports differ from import statements only in case of package imports:
/// the package component may refer to a local import shadowing the package name.
pub fn imported_item_path(
    ty_expr: &TypeExpression,
    parent_path: &ModulePath,
    imports: &Imports,
) -> Option<(ModulePath, Ident)> {
    if let Some(path) = &ty_expr.path {
        match &path.origin {
            PathOrigin::Package(pkg_name) => {
                // the path could be either a package, or referencing an imported module alias.
                let imported_item = imports.iter().find(|(ident, _)| *ident.name() == *pkg_name);

                if let Some((_, ext_item)) = imported_item {
                    // this inline path references an imported item. Example:
                    // import a::b::c as foo; foo::bar::baz() => a::b::c::bar::baz()
                    let mut res = ext_item.path.clone(); // a::b
                    res.push(&ext_item.ident.name()); // c (not foo)
                    let import_path = res.join(path.components.iter().cloned()); // a::b::c::bar
                    Some((import_path, ty_expr.ident.clone()))
                } else {
                    let import_path = parent_path.join_path(path);
                    Some((import_path, ty_expr.ident.clone()))
                }
            }
            _ => {
                let import_path = parent_path.join_path(path);
                Some((import_path, ty_expr.ident.clone()))
            }
        }
    } else if let Some(item) = imports.get(&ty_expr.ident) {
        Some((item.path.clone(), item.ident.clone()))
    } else {
        None
    }
}
