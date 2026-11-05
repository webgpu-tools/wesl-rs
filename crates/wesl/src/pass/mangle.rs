use std::collections::HashSet;

use wgsl_parse::{
    SyntaxNode,
    syntax::{ModulePath, TranslationUnit},
};

use crate::mangler::Mangler;

/// Mangle all declarations in all modules. Should be called after `retarget_idents`.
///
/// Panics if a module is already borrowed.
pub fn mangle(module: &mut TranslationUnit, path: &ModulePath, mangler: &impl Mangler) {
    // overloaded functions share the same identifier, it must be renamed only once.
    let mut mangled = HashSet::new();
    module
        .global_declarations
        .iter_mut()
        .filter_map(|decl| decl.ident())
        .for_each(|mut ident| {
            if mangled.insert(ident.clone()) {
                let new_name = mangler.mangle(path, &ident.name());
                ident.rename(new_name.clone());
            }
        })
}
