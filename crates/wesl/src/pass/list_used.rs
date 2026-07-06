use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use wgsl_parse::{SyntaxNode, syntax::*};

use crate::{Module, visit::Visit};

#[derive(Clone, Debug)]
struct ImportedItem {
    path: ModulePath,
    ident: Ident, // this is the ident's original name before `as` renaming.
    public: bool,
}

type Imports = HashMap<Ident, ImportedItem>;
pub type UsedItems = HashMap<ModulePath, HashSet<String>>;

/// Find declarations used in external modules.
pub fn list_used(module: &Module, used_items: &mut UsedItems) {
    let imports = flatten_imports(&module.syntax.imports, &module.path);

    // const_asserts are always used. we add them if the module has not been analyzed yet.
    if !used_items.contains_key(&module.path) {
        let const_asserts = module
            .syntax
            .global_declarations
            .iter()
            .filter(|decl| decl.is_const_assert());

        for decl in const_asserts {
            run_decl(module, decl, &imports, used_items);
        }
    } else {
        used_items.insert(module.path.clone(), Default::default());
    }

    for ident in &module.used_items {
        let decl = module
            .syntax
            .global_declarations
            .iter()
            .find(|decl| decl.ident().as_ref() == Some(ident));

        if let Some(decl) = decl {
            run_decl(module, decl, &imports, used_items);
        }
    }
}

/// Find declarations used in external modules.
fn run_decl(
    module: &Module,
    decl: &GlobalDeclaration,
    imports: &Imports,
    used_items: &mut UsedItems,
) {
    Visit::<TypeExpression>::visit_rec(decl, &mut |ty_expr| {
        if let Some(import_path) = imported_item_path(ty_expr, &module.path, &imports) {
            used_items
                .entry(import_path)
                .or_default()
                .insert(ty_expr.ident.to_string());
        } else {
            let decl = module
                .syntax
                .global_declarations
                .iter()
                .find(|decl| decl.ident().as_ref() == Some(&ty_expr.ident));

            if let Some(decl) = decl {
                let inserted = used_items
                    .entry(module.path.clone())
                    .or_default()
                    .insert(ty_expr.ident.to_string());

                if inserted && !module.used_items.contains(&ty_expr.ident) {
                    run_decl(module, decl, imports, used_items);
                }
            }
        }
    });
}

/// Find the normalized module path for an identifier in source, if it refers to an external declaration.
///
/// Inline imports differ from import statements only in case of package imports:
/// the package component may refer to a local import shadowing the package name.
fn imported_item_path(
    ty_expr: &TypeExpression,
    parent_path: &ModulePath,
    imports: &Imports,
) -> Option<ModulePath> {
    if let Some(path) = &ty_expr.path {
        match &path.origin {
            PathOrigin::Package(pkg_name) => {
                // the path could be either a package, of referencing an imported module alias.
                let imported_item = imports.iter().find(|(ident, _)| *ident.name() == *pkg_name);

                if let Some((_, ext_item)) = imported_item {
                    // this inline path references an imported item. Example:
                    // import a::b::c as foo; foo::bar::baz() => a::b::c::bar::baz()
                    let mut res = ext_item.path.clone(); // a::b
                    res.push(&ext_item.ident.name()); // c
                    Some(res.join(path.components.iter().cloned()))
                } else {
                    Some(parent_path.join_path(path))
                }
            }
            _ => Some(parent_path.join_path(path)),
        }
    } else if let Some(item) = imports.get(&ty_expr.ident) {
        Some(item.path.clone())
    } else {
        None
    }
}

/// Flatten imports to a list.
fn flatten_imports(imports: &[ImportStatement], path: &ModulePath) -> Imports {
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
