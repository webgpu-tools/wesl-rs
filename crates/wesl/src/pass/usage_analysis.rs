use std::collections::{HashMap, HashSet, hash_map::Entry};

use itertools::Itertools;
use wgsl_parse::{SyntaxNode, syntax::*};

use crate::{pipeline::Module, visit::Visit};

#[derive(Clone, Debug)]
pub struct ImportedItem {
    pub path: ModulePath,
    pub ident: Ident, // this is the ident's original name before `as` renaming.
    pub public: bool,
}

pub struct UsedItems {
    used_items: HashMap<ModulePath, HashSet<Ident>>,
}

impl UsedItems {
    pub fn new() -> Self {
        Self {
            used_items: Default::default(),
        }
    }

    pub fn contains_module(&self, path: &ModulePath) -> bool {
        self.used_items.contains_key(path)
    }

    pub fn contains_ident(&self, path: &ModulePath, ident: &Ident) -> bool {
        self.used_items
            .get(path)
            .is_some_and(|items| items.contains(ident))
    }

    pub fn contains_name(&self, path: &ModulePath, name: &str) -> bool {
        self.used_items
            .get(path)
            .is_some_and(|items| items.iter().any(|ident| &**ident.name() == name))
    }

    pub fn insert_module(&mut self, path: ModulePath, idents: HashSet<Ident>) -> bool {
        match self.used_items.entry(path) {
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                entry.insert(idents);
                true
            }
        }
    }

    pub fn insert_ident(&mut self, path: ModulePath, ident: Ident) -> bool {
        let entry = self.used_items.entry(path.clone()).or_default();
        let inserted = entry.insert(ident);
        inserted
    }

    pub fn remove_module(&mut self, path: &ModulePath) -> bool {
        self.used_items.remove(path).is_some()
    }

    pub fn is_empty(&mut self) -> bool {
        self.used_items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ModulePath, &HashSet<Ident>)> {
        self.used_items.iter()
    }
}

/// Find declarations used in external modules.
///
/// "used" means referenced transitively from the program entry points.
///
/// The function call adds identifiers to the `used` parameter.
pub fn usage_analysis(
    module: &Module,
    decl_name: &str,
    already_used: &mut UsedItems,
    newly_used: &mut UsedItems,
) {
    // const_asserts are always used. we add them if the module has not been analyzed yet.
    if already_used.contains_module(&module.path) {
        already_used.insert_module(module.path.clone(), Default::default());
        let const_asserts = module
            .syntax
            .global_declarations
            .iter()
            .filter(|decl| decl.is_const_assert());

        for decl in const_asserts {
            decl_usage_analysis(module, decl, already_used, newly_used);
        }
    }

    ident_usage_analysis(decl_name, module, already_used, newly_used);
}

/// Find identifiers used by the declaration named `name`.
fn ident_usage_analysis(
    name: &str,
    module: &Module,
    already_used: &mut UsedItems,
    newly_used: &mut UsedItems,
) {
    if !already_used.contains_name(&module.path, name) {
        let decl = module
            .syntax
            .global_declarations
            .iter()
            .find(|decl| decl.ident().is_some_and(|ident| &**ident.name() == name));

        if let Some(decl) = decl {
            already_used.insert_ident(
                module.path.clone(),
                decl.ident().unwrap(/* SAFETY: we found the declaration by name, so it has a name */),
            );
            decl_usage_analysis(module, decl, already_used, newly_used);
        } else {
            // TODO: error when the ident is not found?
        }
    }
}

/// Find identifiers used by a declaration.
fn decl_usage_analysis(
    module: &Module,
    decl: &GlobalDeclaration,
    already_used: &mut UsedItems,
    newly_used: &mut UsedItems,
) {
    Visit::<TypeExpression>::visit_rec(decl, &mut |ty_expr| {
        // this ident refers an imported item, we add it to the list of used items.
        if let Some(import_path) = imported_item_path(ty_expr, &module.path, &module.imports)
            && !already_used.contains_name(&import_path, &**ty_expr.ident.name())
        {
            newly_used.insert_ident(import_path, ty_expr.ident.clone());
        }
        // this ident refers a local declaration, we analyze it recursively.
        else {
            ident_usage_analysis(&ty_expr.ident.name(), module, already_used, newly_used);
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
