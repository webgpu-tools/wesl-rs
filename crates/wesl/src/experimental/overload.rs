use std::collections::HashMap;

use itertools::Itertools;
use wgsl_parse::syntax::{
    Function, GlobalDeclaration, Ident, Import, TranslationUnit, TypeExpression,
};

use crate::{
    import::{absolute_resource, imported_resources, normalize_resource},
    visit::Visit,
    Mangler, Resource,
};

fn normalize_ty(
    ty: &TypeExpression,
    parent_res: &Resource,
    imports: &HashMap<Ident, (Resource, Ident)>,
) -> TypeExpression {
    let mut ty = ty.clone();
    if let Some(path) = &ty.path {
        let res = normalize_resource(path, parent_res, imports);
        ty.path = Some(res.path().to_path_buf());
    } else {
        ty.path = Some(parent_res.path().to_path_buf());
    }
    ty
}

pub(crate) fn mangle(wesl: &mut TranslationUnit, resource: &Resource, mangler: &impl Mangler) {
    let imports = imported_resources(&wesl.imports, resource);

    for decl in &mut wesl.global_declarations {
        if let GlobalDeclaration::Function(decl) = decl {
            // TODO: type aliases
            let sig = decl
                .parameters
                .iter()
                .map(|p| normalize_ty(&p.ty, resource, &imports)).collect_vec()
            let res = mangler.mangle_signature(decl.ident.name().as_str(), &sig);
        }
    }
}
