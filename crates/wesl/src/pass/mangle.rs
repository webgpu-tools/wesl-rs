use wgsl_parse::{
    SyntaxNode,
    syntax::{
        DeclarationKind, GlobalDeclaration, GlobalDeclarationNode, ModulePath, TranslationUnit,
    },
};

use crate::mangler::Mangler;

/// Whether a declaration is host-visible: the host refers to it by name in the compiled
/// WGSL, so it must be neither mangled nor stripped. Currently, only pipeline-overridable
/// constants.
pub fn is_host_visible(decl: &GlobalDeclarationNode) -> bool {
    matches!(
        decl.node(),
        GlobalDeclaration::Declaration(decl) if decl.kind == DeclarationKind::Override
    )
}

/// Mangle all declarations in a module except host-visible ones.
///
/// Should be called after `retarget_idents`.
///
/// Panics if a module is already borrowed.
pub fn mangle(module: &mut TranslationUnit, path: &ModulePath, mangler: &impl Mangler) {
    module
        .global_declarations
        .iter_mut()
        .filter(|decl| !is_host_visible(decl))
        .filter_map(|decl| decl.ident())
        .for_each(|mut ident| {
            let new_name = mangler.mangle(path, &ident.name());
            ident.rename(new_name.clone());
        })
}
