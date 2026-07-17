use std::collections::HashSet;

use wesl_macros::query;
use wgsl_parse::SyntaxNode;
use wgsl_parse::syntax::{
    Expression, ExpressionNode, FunctionCall, GlobalDeclaration, Ident, ImportContent,
    TranslationUnit, TypeExpression,
};
use wgsl_types::idents::{BUILTIN_CONSTRUCTOR_NAMES, BUILTIN_FUNCTION_NAMES};

use crate::{
    error::{Diagnostic, Error, ValidateError},
    idents::builtin_ident,
    visit::Visit,
};

type E = ValidateError;

/// An identifier is linked to a declaration if:
/// * its use-count is greater than 1
/// * OR it is a built-in name
///
/// Note that this function could be simplified if we didn't care about the diagnostics metadata (declaration and expression)
fn check_defined_symbols(wesl: &TranslationUnit) -> Result<(), Diagnostic<Error>> {
    fn check_ty(ty: &TypeExpression) -> Result<(), Diagnostic<Error>> {
        if ty.path.is_none()
            && ty.ident.use_count() == 1
            && builtin_ident(&ty.ident.name()).is_none()
            // `_` is only valid for phony assignments
            && *ty.ident.name() != "_"
        {
            Err(E::UndefinedSymbol(ty.ident.to_string()).into())
        } else {
            for arg in ty.template_args.iter().flatten() {
                check_expr(&arg.expression)?;
            }
            Ok(())
        }
    }
    fn check_expr(expr: &ExpressionNode) -> Result<(), Diagnostic<Error>> {
        if let Expression::TypeOrIdentifier(ty) = expr.node() {
            check_ty(ty).map_err(|d| d.with_span(expr.span()))
        } else if let Expression::FunctionCall(call) = expr.node() {
            check_ty(&call.ty).map_err(|d| d.with_span(expr.span()))?;
            for expr in &call.arguments {
                check_expr(expr)?;
            }
            Ok(())
        } else {
            for expr in Visit::<ExpressionNode>::visit(expr.node()) {
                check_expr(expr)?;
            }
            Ok(())
        }
    }
    fn check_decl(decl: &GlobalDeclaration) -> Result<(), Diagnostic<Error>> {
        let decl_name = decl.ident().map(|ident| ident.name().to_string());
        for expr in Visit::<ExpressionNode>::visit(decl) {
            check_expr(expr).map_err(|mut d| {
                d.detail.declaration = decl_name.clone();
                d
            })?;
        }

        // those are the attributes that don't have an expression as parent.
        // unfortunately the diagnostic won't have a span :(
        for ty in query!(decl.{
            GlobalDeclaration::Declaration.ty.[],
            GlobalDeclaration::TypeAlias.ty,
            GlobalDeclaration::Struct.members.[].ty,
            GlobalDeclaration::Function.{ parameters.[].ty, return_type.[] }
        }) {
            check_ty(ty).map_err(|mut d| {
                d.detail.declaration = decl_name.clone();
                d
            })?;
        }
        Ok(())
    }

    for decl in &wesl.global_declarations {
        check_decl(decl)?;
    }
    Ok(())
}

fn check_function_calls(wesl: &TranslationUnit) -> Result<(), Diagnostic<Error>> {
    fn check_call(call: &FunctionCall, ident: &Ident, wesl: &TranslationUnit) -> Result<(), E> {
        let decl = wesl
            .global_declarations
            .iter()
            .find(|decl| decl.ident().is_some_and(|id| &id == ident))
            .map(|decl| decl.node());

        match decl {
            Some(GlobalDeclaration::Function(decl)) => {
                if call.arguments.len() != decl.parameters.len() {
                    return Err(E::ParamCount(
                        ident.to_string(),
                        decl.parameters.len(),
                        call.arguments.len(),
                    ));
                }
            }
            Some(GlobalDeclaration::Struct(decl)) => {
                if call.arguments.len() != decl.members.len() && !call.arguments.is_empty() {
                    return Err(E::ParamCount(
                        ident.to_string(),
                        decl.members.len(),
                        call.arguments.len(),
                    ));
                }
            }
            Some(GlobalDeclaration::TypeAlias(decl)) => {
                if decl.ty.template_args.is_some() {
                    // TODO: check args for builtin functions
                } else {
                    check_call(call, &decl.ty.ident, wesl)?;
                }
            }
            Some(_) => return Err(E::NotCallable(ident.to_string())),
            None => {
                if BUILTIN_FUNCTION_NAMES
                    .iter()
                    .any(|name| name == &*ident.name())
                {
                    // TODO: check args for builtin functions
                } else if BUILTIN_CONSTRUCTOR_NAMES
                    .iter()
                    .any(|name| name == &*ident.name())
                {
                    // TODO: check args for builtin constructors
                } else {
                    // the ident is not a global declaration, it must be a local variable.
                    return Err(E::NotCallable(ident.to_string()));
                }
            }
        };
        Ok(())
    }
    fn check_expr(expr: &Expression, wesl: &TranslationUnit) -> Result<(), E> {
        if let Expression::FunctionCall(call) = expr {
            check_call(call, &call.ty.ident, wesl)?;
        }
        Ok(())
    }
    for decl in &wesl.global_declarations {
        for expr in Visit::<ExpressionNode>::visit(decl.node()) {
            check_expr(expr, wesl).map_err(|e| {
                let mut err = Diagnostic::from(e);
                err.detail.span = Some(expr.span());
                err.detail.declaration = decl.ident().map(|id| id.name().to_string());
                err
            })?;
        }
    }
    Ok(())
}

fn check_duplicate_decl(wesl: &TranslationUnit) -> Result<(), Diagnostic<Error>> {
    let mut unique = HashSet::new();

    fn check_ident(id: &Ident, unique: &mut HashSet<String>) -> Result<(), Diagnostic<Error>> {
        if unique.contains(id.name().as_str()) {
            Err(Diagnostic::from(E::Duplicate(id.to_string())).with_declaration(id.to_string()))
        } else {
            unique.insert(id.to_string());
            Ok(())
        }
    }

    fn check_import_content(
        cont: &ImportContent,
        unique: &mut HashSet<String>,
    ) -> Result<(), Diagnostic<Error>> {
        match cont {
            ImportContent::Item(item) => {
                let id = item.rename.as_ref().unwrap_or(&item.ident);
                check_ident(id, unique)?;
            }
            ImportContent::Collection(coll) => {
                for item in coll {
                    check_import_content(&item.content, unique)?;
                }
            }
        }
        Ok(())
    }

    for import in &wesl.imports {
        if import
            .attributes()
            .iter()
            .any(|attr| attr.is_if() || attr.is_elif() || attr.is_else())
        {
            // we skip checking declarations that have conditional compilation flags.
            continue;
        }
        check_import_content(&import.content, &mut unique)?;
    }

    for decl in &wesl.global_declarations {
        if decl
            .attributes()
            .iter()
            .any(|attr| attr.is_if() || attr.is_elif() || attr.is_else())
        {
            // we skip checking declarations that have conditional compilation flags.
            continue;
        }
        if let Some(id) = decl.ident() {
            check_ident(&id, &mut unique)?;
        }
    }
    Ok(())
}

fn check_cycles(wesl: &TranslationUnit) -> Result<(), Diagnostic<Error>> {
    fn check_decl(
        id: &Ident,
        decl: &GlobalDeclaration,
        unique: &mut HashSet<Ident>,
        wesl: &TranslationUnit,
    ) -> Result<(), E> {
        let mut res = Ok(());

        Visit::<TypeExpression>::visit_rec(decl, &mut |ty| {
            if res.is_ok() {
                if ty.ident == *id {
                    res = Err(E::Cycle(id.to_string(), decl.ident().unwrap().to_string()));
                } else if unique.insert(ty.ident.clone())
                    && let Some(decl) = wesl
                        .global_declarations
                        .iter()
                        .find(|decl| decl.ident().as_ref() == Some(&ty.ident))
                {
                    res = check_decl(id, decl, unique, wesl);
                }
            }
        });

        res
    }
    for decl in &wesl.global_declarations {
        if let Some(id) = decl.ident() {
            let mut unique = HashSet::new();
            check_decl(&id, decl, &mut unique, wesl)
                .map_err(|e| Diagnostic::from(e).with_declaration(id.to_string()))?;
        }
    }
    Ok(())
}

/// Validate an intermediate WESL module.
///
/// This function only checks that a WESL module is valid on its own, without looking at
/// external modules (imports).
///
/// It currently does not validate a lot. It checks for:
/// * Defined declarations: all identifiers refer to a user declaration, import or
///   built-in name.
/// * Duplicate declarations: declarations in the same scope cannot have the same name.
///   (except for unresolved conditional compilation)
/// * Cyclic declarations: no cycles are allowed in declarations.
pub fn validate_wesl(wesl: &TranslationUnit) -> Result<(), Diagnostic<Error>> {
    check_defined_symbols(wesl)?;
    check_duplicate_decl(wesl)?;
    check_cycles(wesl)?;
    Ok(())
}

/// Validate the final output (valid WGSL).
///
/// It currently does not validate a lot. It checks for:
/// * Defined declarations: all identifiers refer to a user declaration or built-in name.
/// * Duplicate declarations: declarations in the same scope cannot have the same name.
/// * Cyclic declarations: no cycles are allowed in declarations.
/// * Function calls: call expressions must refer to a function or a type constructor.
///   Check the number of arguments but not their type.
pub fn validate_wgsl(wgsl: &TranslationUnit) -> Result<(), Diagnostic<Error>> {
    check_defined_symbols(wgsl)?;
    check_duplicate_decl(wgsl)?;
    check_cycles(wgsl)?;
    check_function_calls(wgsl)?;
    Ok(())
}
