use crate::{error::Error, pass::Visit};

use wgsl_parse::syntax::*;

/// Performs conversions on the final syntax tree to make it more compatible with WGSL
/// implementations like Naga, catch errors early and perform optimizations.
///
/// Currently, `lower` performs the following transforms:
/// * remove aliases (inlined)
/// * remove consts (inlined)
/// * remove deprecated, non-standard attributes
/// * remove import declarations
///
/// with the `eval` feature flag enabled, it performs additional transforms:
/// * evaluate const-expressions (including calls to const functions)
/// * remove unreachable code paths after const-evaluation
/// * remove function call statements to const functions (no side-effects)
/// * make implicit conversions from abstract types explicit (using conversion rank)
///
/// Customizing this behavior is not possible currently. The following transforms may
/// be available in the future:
/// * make variable types explicit
/// * remove unused variables / code with no side-effects
pub fn lower(module: &mut TranslationUnit) -> Result<(), Error> {
    module.imports.clear();

    for attrs in Visit::<Attributes>::visit_mut(module) {
        attrs.retain(|attr| {
            !matches!(attr.node(),
            Attribute::Custom(CustomAttribute { name, .. }) if name == "generic")
        })
    }

    #[cfg(not(feature = "eval"))]
    {
        // these are redundant with eval::lower.
        remove_type_aliases(module);
        remove_global_consts(module);
    }
    #[cfg(feature = "eval")]
    {
        use crate::error::Diagnostic;
        use crate::eval::{Context, Exec, Lower, mark_functions_const};
        use wgsl_parse::SyntaxNode;
        mark_functions_const(module);

        // we want to drop wesl2 at the end of the block for idents use_count
        {
            let module2 = module.clone();
            let mut ctx = Context::new(&module2);
            module
                .exec(&mut ctx) // populate the ctx with module-scope declarations
                .map_err(|e| Diagnostic::from(e).with_ctx(&ctx))?;
            module
                .lower(&mut ctx)
                .map_err(|e| Diagnostic::from(e).with_ctx(&ctx))?;
        }

        // remove `@const` attributes.
        for decl in &mut module.global_declarations {
            if let GlobalDeclaration::Function(decl) = decl.node_mut() {
                decl.retain_attributes_mut(|attr| *attr != Attribute::Const);
            }
        }
    }
    Ok(())
}

/// Eliminate all type aliases.
/// Naga doesn't like this: `alias T = u32; vec<T>`
#[allow(unused)]
fn remove_type_aliases(wesl: &mut TranslationUnit) {
    let take_next_alias = |wesl: &mut TranslationUnit| {
        let index = wesl
            .global_declarations
            .iter()
            .position(|decl| matches!(decl.node(), GlobalDeclaration::TypeAlias(_)));
        index.map(|index| {
            let decl = wesl.global_declarations.swap_remove(index);
            match decl.into_inner() {
                GlobalDeclaration::TypeAlias(alias) => alias,
                _ => unreachable!(),
            }
        })
    };

    while let Some(mut alias) = take_next_alias(wesl) {
        // we rename the alias and all references to its type expression,
        // and drop the alias declaration.
        alias.ident.rename(format!("{}", alias.ty));
    }
}

/// Eliminate all const-declarations.
///
/// Replace usages of the const-declaration with its expression.
///
/// # Panics
/// panics if the const-declaration is ill-formed, i.e. has no initializer.
#[allow(unused)]
fn remove_global_consts(wesl: &mut TranslationUnit) {
    let take_next_const = |wesl: &mut TranslationUnit| {
        let index = wesl.global_declarations.iter().position(|decl| {
            matches!(
                decl.node(),
                GlobalDeclaration::Declaration(Declaration {
                    kind: DeclarationKind::Const,
                    ..
                })
            )
        });
        index.map(|index| {
            let decl = wesl.global_declarations.swap_remove(index);
            match decl.into_inner() {
                GlobalDeclaration::Declaration(d) => d,
                _ => unreachable!(),
            }
        })
    };

    while let Some(mut decl) = take_next_const(wesl) {
        // we rename the const and all references to its expression in parentheses,
        // and drop the const declaration.
        decl.ident
            .rename(format!("({})", decl.initializer.unwrap()));
    }
}
