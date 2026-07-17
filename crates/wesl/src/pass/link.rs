use wgsl_parse::{SyntaxNode, syntax::TranslationUnit};

use crate::{Module, pass::UsedItems};

/// Merge all declarations into a single module.
///
/// If `used_items` is set, the final module includes only used items. Otherwise, it
/// includes all declarations.
pub fn link(modules: &[Module], used_items: Option<&UsedItems>) -> TranslationUnit {
    let mut syntax = TranslationUnit::default();
    for module in modules {
        if let Some(used_items) = &used_items {
            syntax.global_declarations.extend(
                module
                    .syntax
                    .global_declarations
                    .iter()
                    .filter(|decl| {
                        decl.is_const_assert()
                            || decl
                                .ident()
                                .is_some_and(|id| used_items.contains_ident(&module.path, &id))
                    })
                    .cloned(),
            );
        } else {
            syntax
                .global_declarations
                .extend(module.syntax.global_declarations.clone());
        }
        syntax
            .global_directives
            .extend(module.syntax.global_directives.clone());
    }
    // TODO: <https://github.com/wgsl-tooling-wg/wesl-spec/issues/71>
    // currently the behavior is:
    // * include all directives used (if strip)
    // * include all directives (if not strip)
    syntax.global_directives.dedup();
    syntax
}
