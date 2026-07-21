use std::collections::{HashMap, HashSet, hash_map::Entry};
use wgsl_parse::{SyntaxNode, syntax::*};

use crate::pass::{Imports, Visit, flatten_imports, imported_item_path};

pub struct Module {
    pub syntax: TranslationUnit,
    pub path: ModulePath,
    pub imports: Imports,
}

impl Module {
    pub fn new(path: ModulePath, syntax: TranslationUnit) -> Self {
        let imports = flatten_imports(&syntax.imports, &path);
        Self {
            syntax,
            path,
            imports,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct UsedItems {
    /// Module declarations used.
    used_items: HashMap<ModulePath, HashSet<Ident>>,
}

impl UsedItems {
    pub fn new() -> Self {
        Self {
            used_items: Default::default(),
        }
    }

    pub fn get(&self, path: &ModulePath) -> Option<&HashSet<Ident>> {
        self.used_items.get(path)
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

    /// Returns true if inserted.
    pub fn insert_module(&mut self, path: ModulePath, idents: HashSet<Ident>) -> bool {
        match self.used_items.entry(path) {
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                entry.insert(idents);
                true
            }
        }
    }

    /// Returns true if inserted.
    pub fn insert_ident(&mut self, path: ModulePath, ident: Ident) -> bool {
        let entry = self.used_items.entry(path.clone()).or_default();
        let inserted = entry.insert(ident);
        inserted
    }

    /// Returns true if deleted.
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

/// Find declarations used in external modules which this module depends on, no matter what.
///
/// Currently, only items referenced by module-scope `const_assert`s are always included.
///
/// See [`usage_analysis`].
pub fn root_usage_analysis(
    module: &Module,
    already_used: &mut UsedItems,
    to_analyze: &mut UsedItems,
) {
    if already_used.contains_module(&module.path) {
        return;
    }

    already_used.insert_module(module.path.clone(), Default::default());
    let const_asserts = module
        .syntax
        .global_declarations
        .iter()
        .filter(|decl| decl.is_const_assert());

    for decl in const_asserts {
        decl_usage_analysis(module, decl, already_used, to_analyze);
    }
}

/// Find declaration names which a local declaration depends on.
///
/// Adds *external* referenced idents to the `to_analyze` parameter.
/// Perform usage analysis recursively with *local* referenced idents, and adds them to `already_used`.
/// So at the end of the call, `to_analyze` contains incomplete usage analysis which needs to continue
/// in a separate module. `already_used` contains finished analysis.
pub fn usage_analysis(
    module: &Module,
    decl_name: &str,
    already_used: &mut UsedItems,
    to_analyze: &mut UsedItems,
) {
    if !already_used.contains_name(&module.path, decl_name) {
        let decl = module.syntax.global_declarations.iter().find(|decl| {
            decl.ident()
                .is_some_and(|ident| &**ident.name() == decl_name)
        });

        if let Some(decl) = decl {
            decl_usage_analysis(module, decl, already_used, to_analyze);
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
    if decl
        .ident()
        .is_some_and(|ident| !already_used.insert_ident(module.path.clone(), ident))
    {
        // the ident has already been analyzed.
        return;
    }

    Visit::<TypeExpression>::visit_rec(decl, &mut |ty_expr| {
        // this ident refers an imported item, we add it to the list of used items.
        if let Some((import_path, import_ident)) =
            imported_item_path(ty_expr, &module.path, &module.imports)
            && !already_used.contains_name(&import_path, &**ty_expr.ident.name())
        {
            newly_used.insert_ident(import_path, import_ident);
        }
        // this ident refers a local declaration, we analyze it recursively.
        else {
            // look up a global decl with the same ident - not just the same name,
            // because it could be shadowed by a local decl
            let decl = module
                .syntax
                .global_declarations
                .iter()
                .find(|decl| decl.ident().is_some_and(|ident| ident == ty_expr.ident));

            if let Some(decl) = decl {
                decl_usage_analysis(module, decl, already_used, newly_used);
            }
        }
    });
}
