mod mangle;

use itertools::Itertools;
use thiserror::Error;
use wgsl_parse::{SyntaxNode, span::Spanned, syntax::*};

use crate::visit::Visit;

/// Generics error (experimental)
#[derive(Clone, Debug, Error)]
pub enum GenericsError {
    #[error("template not allowed on a generic parameter")]
    DisallowedTemplate,
}

type E = GenericsError;

pub fn generate_variants(wesl: &mut TranslationUnit) -> Result<(), E> {
    let mut new_decls = Vec::new();
    for decl in &wesl.global_declarations {
        let decl_span = decl.span();
        if let GlobalDeclaration::Function(decl) = decl.node() {
            let ty_constrs = decl
                .attributes
                .iter()
                .rev()
                .filter_map(|attr| match attr.node() {
                    Attribute::Type(t) => Some(t),
                    _ => None,
                });

            let variants = ty_constrs
                .map(|t| t.variants.iter().map(|v| (&t.ident, v)))
                .multi_cartesian_product();

            for variant in variants {
                if variant.is_empty() {
                    break;
                }
                let mut decl = decl.clone();
                decl.attributes
                    .retain(|attr| !matches!(attr.node(), Attribute::Type(_)));

                // rename uses of the generic args with the concrete variant
                for (old_id, new_ty) in &variant {
                    let new_id = Ident::new(format!("{new_ty}"));
                    for ty in Visit::<TypeExpression>::visit_mut(&mut decl) {
                        if &ty.ident == *old_id {
                            if ty.template_args.is_some() {
                                return Err(E::DisallowedTemplate);
                            }
                            ty.ident = new_id.clone();
                        }
                    }
                }

                let constraints = variant.iter().map(|&(name, ty)| TypeConstraint {
                    ident: name.clone(),
                    variants: vec![ty.clone()],
                });

                // evaluate type attributes
                for ty in constraints {
                    // this is future-proofing. We'll want to generate fewer variants in
                    // the future by grouping them.
                    eval_ty_attrs(&mut decl.parameters, &ty)?;
                    stat_eval_ty_attrs(&mut decl.body.statements, &ty)?;
                }

                // remove evaluated type attributes
                for stmt in &mut decl.body.statements {
                    for attrs in Visit::<Attributes>::visit_mut(stmt.node_mut()) {
                        attrs.retain(|attr| match attr.node() {
                            Attribute::Type(c) => !c.variants.is_empty(),
                            _ => true,
                        })
                    }
                }

                let signature = decl.parameters.iter().map(|p| p.ty.clone()).collect_vec();

                let new_name = mangle::mangle(&decl.ident.name(), &signature);
                decl.ident = Ident::new(new_name);
                new_decls.push(Spanned::new(decl.into(), decl_span));
            }
        }
    }

    // remove generic function declarations
    wesl.global_declarations.retain(|decl| !matches!(decl.node(), GlobalDeclaration::Function(f) if f.attributes.iter().any(|attr| attr.is_type())));

    // add generic variants
    wesl.global_declarations.extend(new_decls);

    Ok(())
}

pub fn replace_calls(wesl: &mut TranslationUnit) -> Result<(), E> {
    let idents = wesl
        .global_declarations
        .iter()
        .filter_map(|decl| decl.ident())
        .collect_vec();
    for expr in Visit::<ExpressionNode>::visit_mut(wesl) {
        if let Expression::FunctionCall(f) = expr.node_mut()
            && let Some(args) = &f.ty.template_args
        {
            let signature = args
                .iter()
                .map(|arg| match arg.expression.node() {
                    Expression::Literal(_) => todo!("literal generics"),
                    Expression::TypeOrIdentifier(ty) => ty.clone(),
                    _ => panic!("invalid template arg"),
                })
                .collect_vec();

            let new_name = mangle::mangle(&f.ty.ident.name(), &signature);
            f.ty.ident = idents
                .iter()
                .find(|ident| *ident.name() == new_name)
                .unwrap()
                .clone();
            f.ty.template_args = None;
        }
        // TODO recursive
        // expr.visit_mut()
    }
    Ok(())
}

fn eval_ty_attr(opt_node: &mut Option<impl SyntaxNode>, ty: &TypeConstraint) -> Result<(), E> {
    if let Some(node) = opt_node {
        let vars = node
            .attributes_mut()
            .iter_mut()
            .find_map(|attr| match attr.node() {
                Attribute::Type(TypeConstraint {
                    ident: name,
                    variants,
                }) if name == &ty.ident => Some(variants),
                _ => None,
            });

        if let Some(vars) = vars {
            let keep = ty.variants.iter().all(|ty| vars.contains(ty));
            if !keep {
                *opt_node = None;
            }
        }
    }
    Ok(())
}

fn eval_ty_attrs(nodes: &mut Vec<impl SyntaxNode>, ty: &TypeConstraint) -> Result<(), E> {
    let retains = nodes
        .iter()
        .map(|node| {
            let vars = node.attributes().iter().find_map(|attr| match attr.node() {
                Attribute::Type(TypeConstraint {
                    ident: name,
                    variants,
                }) if name == &ty.ident => Some(variants),
                _ => None,
            });

            if let Some(vars) = vars {
                ty.variants.iter().all(|ty| vars.contains(ty))
            } else {
                true
            }
        })
        .collect_vec();

    let retains = nodes
        .iter_mut()
        .zip(retains)
        .map(|(node, keep)| {
            let ty_attr = node
                .attributes_mut()
                .iter_mut()
                .find_map(|attr| match attr.node_mut() {
                    Attribute::Type(TypeConstraint {
                        ident: name,
                        variants,
                    }) if name == &ty.ident => Some(variants),
                    _ => None,
                });
            if let Some(ty_attr) = ty_attr {
                ty_attr.clear();
                keep
            } else {
                true
            }
        })
        .collect_vec();

    let mut it = retains.iter();
    nodes.retain(|_| *it.next().unwrap());
    Ok(())
}

fn stat_eval_ty_attrs(statements: &mut Vec<StatementNode>, ty: &TypeConstraint) -> Result<(), E> {
    fn rec_one(stmt: &mut StatementNode, ty: &TypeConstraint) -> Result<(), GenericsError> {
        match stmt.node_mut() {
            Statement::Compound(stmt) => rec(&mut stmt.statements, ty)?,
            Statement::If(stmt) => {
                rec(&mut stmt.if_clause.body.statements, ty)?;
                for elif in &mut stmt.else_if_clauses {
                    rec(&mut elif.body.statements, ty)?;
                }
                if let Some(el) = &mut stmt.else_clause {
                    rec(&mut el.body.statements, ty)?;
                }
            }
            Statement::Switch(stmt) => {
                eval_ty_attrs(&mut stmt.clauses, ty)?;
                for clause in &mut stmt.clauses {
                    rec(&mut clause.body.statements, ty)?;
                }
            }
            Statement::Loop(stmt) => {
                rec(&mut stmt.body.statements, ty)?;
                eval_ty_attr(&mut stmt.continuing, ty)?;
                if let Some(cont) = &mut stmt.continuing {
                    rec(&mut cont.body.statements, ty)?;
                    eval_ty_attr(&mut cont.break_if, ty)?;
                }
                rec(&mut stmt.body.statements, ty)?;
            }
            Statement::For(stmt) => {
                if let Some(init) = &mut stmt.initializer {
                    rec_one(&mut *init, ty)?
                }
                if let Some(updt) = &mut stmt.update {
                    rec_one(&mut *updt, ty)?
                }
                rec(&mut stmt.body.statements, ty)?
            }
            Statement::While(stmt) => rec(&mut stmt.body.statements, ty)?,
            _ => (),
        };
        Ok(())
    }
    fn rec(stats: &mut Vec<StatementNode>, ty: &TypeConstraint) -> Result<(), E> {
        eval_ty_attrs(stats, ty)?;
        for stmt in stats {
            rec_one(stmt, ty)?;
        }
        Ok(())
    }
    rec(statements, ty)
}
