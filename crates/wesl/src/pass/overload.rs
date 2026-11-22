use std::collections::{HashMap, HashSet};

use wgsl_parse::{SyntaxNode, syntax::*};

use crate::{
    Mangler,
    error::{Diagnostic, Error},
    eval::{
        Context, EvalError, OverloadSets, fn_signature, init_module_scope, mark_functions_const,
        resolve_calls,
    },
    pass::Visit,
};

/// Resolve calls to overloaded functions. Should be called on the linked module, after
/// mangling.
///
/// Functions with the same name declared in the same module are overloads. They share the
/// same identifier until this pass, which gives each overload a unique name with
/// [`Mangler::mangle_overload`] and points each function call to the overload selected by
/// the [WGSL overload resolution algorithm](https://www.w3.org/TR/WGSL/#overload-resolution-section).
///
/// If `keep` is set, overloads that are not called are removed, unless the overloaded
/// function identifier is in `keep`.
pub fn resolve_overloads(
    module: &mut TranslationUnit,
    mangler: &impl Mangler,
    keep: Option<&HashSet<Ident>>,
) -> Result<(), Error> {
    // overloads share the same identifier (see `retarget_idents`).
    let mut groups: Vec<(Ident, Vec<usize>)> = Vec::new();
    for (i, decl) in module.global_declarations.iter().enumerate() {
        if let GlobalDeclaration::Function(decl) = decl.node() {
            match groups.iter_mut().find(|(id, _)| *id == decl.ident) {
                Some((_, indices)) => indices.push(i),
                None => groups.push((decl.ident.clone(), vec![i])),
            }
        }
    }
    groups.retain(|(_, indices)| indices.len() > 1);

    if groups.is_empty() {
        return Ok(());
    }

    let mut taken_names = module
        .global_declarations
        .iter()
        .filter_map(|decl| decl.ident())
        .filter(|id| !groups.iter().any(|(group_id, _)| group_id == id))
        .map(|id| id.to_string())
        .collect::<HashSet<_>>();

    // the evaluation context refers to a copy of the module before any rewrite.
    let mut source = module.clone();
    mark_functions_const(&mut source);
    let mut ctx = Context::new(&source);
    init_module_scope(&mut ctx);

    let mut sets = OverloadSets::new();

    for (ident, indices) in &groups {
        let mut overloads = Vec::new();
        for i in indices {
            let GlobalDeclaration::Function(decl) = module.global_declarations[*i].node() else {
                unreachable!("overloads are functions")
            };
            let sig = fn_signature(decl, &mut ctx).map_err(|e| {
                Diagnostic::from(e)
                    .with_ctx(&ctx)
                    .with_declaration(ident.to_string())
            })?;
            if overloads.iter().any(|(s, _)| *s == sig) {
                let err = EvalError::DuplicateOverload(ident.to_string(), sig);
                return Err(Diagnostic::from(err)
                    .with_declaration(ident.to_string())
                    .into());
            }
            let mut name = mangler.mangle_overload(&ident.name(), &sig);
            if taken_names.contains(&name) {
                name = (1..)
                    .map(|n| format!("{name}_{n}"))
                    .find(|name| !taken_names.contains(name))
                    .unwrap(/* SAFETY: infinite iterator */);
            }
            taken_names.insert(name.clone());
            overloads.push((sig, Ident::new(name)));
        }
        sets.insert(ident.clone(), overloads);
    }

    resolve_calls(module, &mut ctx, &sets).map_err(|e| Diagnostic::from(e).with_ctx(&ctx))?;
    drop(ctx);

    // rename the overload declarations
    let mut overloads = HashMap::new();
    for (ident, indices) in &groups {
        let new_idents = &sets[ident];
        for (i, (_, new_ident)) in indices.iter().zip(new_idents) {
            if let GlobalDeclaration::Function(decl) = module.global_declarations[*i].node_mut() {
                decl.ident = new_ident.clone();
                overloads.insert(new_ident.clone(), ident.clone());
            }
        }
    }

    // remove unused overloads
    if let Some(keep) = keep {
        loop {
            let mut used = HashSet::new();
            for decl in &module.global_declarations {
                Visit::<TypeExpression>::visit_rec(decl.node(), &mut |ty| {
                    used.insert(ty.ident.clone());
                });
            }
            let len = module.global_declarations.len();
            module.global_declarations.retain(|decl| match decl.node() {
                GlobalDeclaration::Function(decl) => match overloads.get(&decl.ident) {
                    Some(ident) => keep.contains(ident) || used.contains(&decl.ident),
                    None => true,
                },
                _ => true,
            });
            if module.global_declarations.len() == len {
                break;
            }
        }
    }

    Ok(())
}
