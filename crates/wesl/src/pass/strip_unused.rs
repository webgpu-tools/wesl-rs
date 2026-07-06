use std::collections::HashSet;

use wgsl_parse::{
    SyntaxNode,
    syntax::{Ident, TranslationUnit},
};

/// Remove unused declarations.
pub(crate) fn strip_except(wgsl: &mut TranslationUnit, keep: &HashSet<Ident>) {
    wgsl.global_declarations.retain_mut(|decl| {
        if let Some(id) = decl.ident() {
            keep.contains(&id) || id.use_count() > 1
        } else {
            true
        }
    });
}
