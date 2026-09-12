use wgsl_parse::syntax::{GlobalDeclaration, TranslationUnit, Visibility};

/// Remove visibility markers (`public` / `private`) in a module, i.e. make all declarations
/// package-visible.
pub fn strip_visibility(module: &mut TranslationUnit) {
    for import in &mut module.imports {
        import.visibility = Visibility::Package;
    }

    for decl in &mut module.global_declarations {
        match decl.node_mut() {
            GlobalDeclaration::Declaration(decl) => decl.visibility = Visibility::Package,
            GlobalDeclaration::TypeAlias(decl) => decl.visibility = Visibility::Package,
            GlobalDeclaration::Struct(decl) => decl.visibility = Visibility::Package,
            GlobalDeclaration::Function(decl) => decl.visibility = Visibility::Package,
            GlobalDeclaration::Void
            | GlobalDeclaration::ConstAssert(_)
            | GlobalDeclaration::Compound(_) => {}
        }
    }
}
