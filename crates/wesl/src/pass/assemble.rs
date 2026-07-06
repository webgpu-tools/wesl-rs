use wgsl_parse::{SyntaxNode, syntax::TranslationUnit};

use crate::Module;

/// Merge all declarations into a single module. If the `strip` flag is set, it will
/// copy over only used declarations.
pub fn assemble(modules: &[Module], strip: bool) -> TranslationUnit {
    let mut syntax = TranslationUnit::default();
    for module in modules {
        if strip {
            syntax.global_declarations.extend(
                module
                    .syntax
                    .global_declarations
                    .iter()
                    .filter(|decl| {
                        decl.is_const_assert()
                            || decl
                                .ident()
                                .is_some_and(|id| module.used_items.contains(&id))
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
