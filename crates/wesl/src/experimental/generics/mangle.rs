use itertools::Itertools;
use wgsl_parse::syntax::*;

// use crate::Mangler;

// pub fn mangle(wesl: &mut TranslationUnit, mangler: impl Mangler) {}

// fn mangle_literal(lit: &LiteralExpression) -> String {
//     match lit {
//         LiteralExpression::Bool(l) => format!("B{l}"),
//         LiteralExpression::AbstractInt(_) |
//         LiteralExpression::AbstractFloat(_) => panic!("abstract numeric types not allowed in mangling"),
//         LiteralExpression::I32(l) => format!("I{l}"),
//         LiteralExpression::U32(l) => format!("U{l}"),
//         LiteralExpression::F32(l) => format!("F{l}"),
//         LiteralExpression::F16(l) => format!("H{l}"),
//     }
// }

fn mangle_arg(ty: &TemplateArg) -> String {
    match ty.expression.node() {
        Expression::Literal(_) => {
            panic!("literal template arguments mangling not implemented")
        }
        Expression::TypeOrIdentifier(ty) => mangle_ty(ty),
        _ => panic!("invalid template argument expression type"),
    }
}

fn mangle_ty(ty: &TypeExpression) -> String {
    let args = ty.template_args.as_deref().unwrap_or_default();
    let n1 = ty.ident.name().len();
    let name = &ty.ident;
    let n2 = args.len();
    let args = args.iter().map(mangle_arg).format("");
    format!("{n1}{name}{n2}_{args}")
}

// https://refspecs.linuxbase.org/cxxabi-1.86.html#mangling
pub fn mangle(name: &str, signature: &[TypeExpression]) -> String {
    let n1 = name.len();
    let n2 = signature.len();
    let sig = signature.iter().map(mangle_ty).format("");
    format!("_WESL{n1}{name}{n2}_{sig}")
}
