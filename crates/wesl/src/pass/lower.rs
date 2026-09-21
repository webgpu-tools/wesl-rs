use crate::{error::Diagnostic, pass::Visit};

use wgsl_parse::syntax::*;

/// Performs conversions on the final syntax tree to make it more compatible with WGSL
/// implementations like Naga, catch errors early and perform optimizations.
///
/// Currently, `lower` performs the following transforms:
/// * remove deprecated, non-standard attributes
/// * remove import declarations
/// * evaluate const-expressions (including calls to const functions)
/// * remove unreachable code paths after const-evaluation
/// * remove function call statements to const functions (no side-effects)
/// * make implicit conversions from abstract types explicit (using conversion rank)
///
/// Customizing this behavior is not possible currently. The following transforms may
/// be available in the future:
/// * make variable types explicit
/// * remove unused variables / code with no side-effects
pub fn lower(module: &mut TranslationUnit) -> Result<(), Diagnostic> {
    module.imports.clear();

    for attrs in Visit::<Attributes>::visit_mut(module) {
        attrs.retain(|attr| {
            !matches!(attr.node(),
            Attribute::Custom(CustomAttribute { name, .. }) if name == "generic")
        })
    }

    use wgsl_types::ty_context::TyContext;
    mark_functions_const(module);

    // we want to drop wesl2 at the end of the block for idents use_count
    {
        let module2 = module.clone();
        let mut ty_ctx = TyContext::default();
        let mut ctx = Context::new(&module2, &mut ty_ctx);
        module
            .exec(&mut ctx) // populate the ctx with module-scope declarations
            .map_err(|e| {
                Diagnostic::new(crate::Error::EvalError(e, ctx.ty_ctx.clone_for_error()))
                    .with_ctx(&ctx)
            })?;
        module.lower(&mut ctx).map_err(|e| {
            Diagnostic::new(crate::Error::EvalError(e, ctx.ty_ctx.clone_for_error())).with_ctx(&ctx)
        })?;
    }

    // remove `@const` attributes.
    for decl in &mut module.global_declarations {
        if let GlobalDeclaration::Function(decl) = decl.node_mut() {
            decl.retain_attributes_mut(|attr| *attr != Attribute::Const);
        }
    }
    Ok(())
}
