use crate::error::{CondCompError, Diagnostic, Error};
use std::collections::HashMap;
use wgsl_parse::{SyntaxNode, span::Spanned, syntax::*};

type E = Error;

/// Set the behavior for a feature flag during conditional translation.
///
/// * `Keep` means that the feature flag will be left as-is. This is useful for
///   incremental compilation, e.g. for generating shader variants
/// * `Error` means that unspecified feature flags will trigger a
///   [`CondCompError::UnexpectedFeatureFlag`].
///
/// Default is `Disable`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Feature {
    Enable,
    #[default]
    Disable,
    Keep,
    Error,
}

/// Toggle conditional compilation feature flags.
///
/// Feature flags set to `true` are enabled, and `false` are disabled. Feature flags not
/// present in `flags` are treated according to `default`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub default: Feature,
    pub flags: HashMap<String, Feature>,
}

impl From<bool> for Feature {
    fn from(value: bool) -> Self {
        if value {
            Feature::Enable
        } else {
            Feature::Disable
        }
    }
}

impl Features {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_feature(&mut self, name: impl ToString, value: impl Into<Feature>) {
        self.flags.insert(name.to_string(), value.into());
    }
}

const EXPR_TRUE: Expression = Expression::Literal(LiteralExpression::Bool(true));
const EXPR_FALSE: Expression = Expression::Literal(LiteralExpression::Bool(false));

fn eval_attr(expr: &ExpressionNode, features: &Features) -> Result<Expression, E> {
    eval_attr_impl(expr, features).map_err(|e| Diagnostic::from(e).with_span(expr.span()).into())
}

fn eval_attr_impl(expr: &Expression, features: &Features) -> Result<Expression, E> {
    fn eval_rec(expr: &ExpressionNode, features: &Features) -> Result<Expression, E> {
        eval_attr(expr, features).map_err(|e| Diagnostic::from(e).with_span(expr.span()).into())
    }

    match expr {
        Expression::Literal(LiteralExpression::Bool(_)) => Ok(expr.clone()),
        Expression::Parenthesized(paren) => {
            let expr = eval_rec(&paren.expression, features)?;
            Ok(match expr {
                Expression::Binary(_) => ParenthesizedExpression {
                    expression: Spanned::new(expr, paren.expression.span()),
                }
                .into(),
                _ => expr,
            })
        }
        Expression::Unary(unary) => {
            let operand = eval_rec(&unary.operand, features)?;
            match &unary.operator {
                UnaryOperator::LogicalNegation => {
                    let expr = if operand == EXPR_TRUE {
                        EXPR_FALSE.clone()
                    } else if operand == EXPR_FALSE {
                        EXPR_TRUE.clone()
                    } else {
                        expr.clone()
                    };
                    Ok(expr)
                }
                _ => Err(CondCompError::InvalidExpression(expr.clone()).into()),
            }
        }
        Expression::Binary(binary) => {
            let left = eval_rec(&binary.left, features)?;
            let right = eval_rec(&binary.right, features)?;
            match &binary.operator {
                BinaryOperator::ShortCircuitOr => {
                    let expr = if left == EXPR_TRUE || right == EXPR_TRUE {
                        EXPR_TRUE.clone()
                    } else if left == EXPR_FALSE && right == EXPR_FALSE {
                        left // false
                    } else if left == EXPR_FALSE {
                        right
                    } else if right == EXPR_FALSE {
                        left
                    } else {
                        BinaryExpression {
                            operator: binary.operator,
                            left: Spanned::new(left, binary.left.span()),
                            right: Spanned::new(right, binary.right.span()),
                        }
                        .into()
                    };
                    Ok(expr)
                }
                BinaryOperator::ShortCircuitAnd => {
                    let expr = if left == EXPR_TRUE && right == EXPR_TRUE {
                        left // true
                    } else if left == EXPR_FALSE || right == EXPR_FALSE {
                        EXPR_FALSE.clone()
                    } else if left == EXPR_TRUE {
                        right
                    } else if right == EXPR_TRUE {
                        left
                    } else {
                        BinaryExpression {
                            operator: binary.operator,
                            left: Spanned::new(left, binary.left.span()),
                            right: Spanned::new(right, binary.right.span()),
                        }
                        .into()
                    };
                    Ok(expr)
                }
                _ => Err(CondCompError::InvalidExpression(expr.clone()).into()),
            }
        }
        Expression::TypeOrIdentifier(ty) => {
            if ty.template_args.is_some() {
                return Err(CondCompError::InvalidFeatureFlag(ty.to_string()).into());
            }
            let feat = features
                .flags
                .get(&*ty.ident.name())
                .unwrap_or(&features.default);
            let expr = match feat {
                Feature::Enable => EXPR_TRUE.clone(),
                Feature::Disable => EXPR_FALSE.clone(),
                Feature::Keep => expr.clone(),
                Feature::Error => {
                    return Err(
                        CondCompError::UnexpectedFeatureFlag(ty.ident.name().to_string()).into(),
                    );
                }
            };
            Ok(expr)
        }
        _ => Err(CondCompError::InvalidExpression(expr.clone()).into()),
    }
}

fn get_single_attr(attrs: &mut [AttributeNode]) -> Result<Option<&mut AttributeNode>, E> {
    let mut it = attrs.iter_mut().filter(|attr| {
        matches!(
            attr.node(),
            Attribute::If(_) | Attribute::Elif(_) | Attribute::Else
        )
    });
    let attr = it.next();

    if it.next().is_some() {
        Err(CondCompError::DuplicateIf.into())
    } else {
        Ok(attr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrevEval {
    has_if: bool,
    is_true: bool,
    removed: bool,
}

/// * ensure there is at most one if/elif/else node.
/// * ensure elif/else nodes are preceded by if/elif.
/// * remove the attributes which evaluate to true.
/// * turn elifs into ifs when previous node was deleted.
/// * turn elifs into elses when it evaluates to true.
fn eval_if_attr(
    node: &mut impl SyntaxNode,
    prev: &mut PrevEval,
    features: &Features,
) -> Result<(), E> {
    let span = node.span();
    eval_if_attr_impl(node, prev, features).map_err(|e| {
        if let Some(span) = span {
            Diagnostic::from(e).with_span(span).into()
        } else {
            e
        }
    })
}

fn eval_if_attr_impl(
    node: &mut impl SyntaxNode,
    prev: &mut PrevEval,
    features: &Features,
) -> Result<(), E> {
    let attr = get_single_attr(node.attributes_mut())?;
    if let Some(attr) = attr {
        let mut has_if = false;
        if let Attribute::If(expr) = attr.node_mut() {
            **expr = eval_attr(expr, features)?;
            has_if = true;
            prev.is_true = false;
        } else if let Attribute::Elif(expr) = attr.node_mut() {
            if !prev.has_if {
                return Err(CondCompError::NoPrecedingIf.into());
            } else {
                **expr = eval_attr(expr, features)?;
                has_if = true;
            }
        } else if let Attribute::Else = attr.node()
            && !prev.has_if
        {
            return Err(CondCompError::NoPrecedingIf.into());
        }
        prev.has_if = has_if;
    } else {
        prev.has_if = false;
    }

    let mut remove_node = false;
    let mut remove_attr = false;
    let mut is_true = false;
    node.retain_attributes_mut(|attr| {
        if let Attribute::If(expr) = attr {
            if **expr == EXPR_TRUE {
                remove_attr = true; // if(true) => remove the attribute
                is_true = true;
            } else if **expr == EXPR_FALSE {
                remove_node = true; // if(false) => remove the node
            }
        } else if let Attribute::Elif(expr) = attr {
            if prev.is_true || **expr == EXPR_FALSE {
                remove_node = true;
            } else if **expr == EXPR_TRUE {
                is_true = true;
                if prev.removed {
                    remove_attr = true;
                } else {
                    *attr = Attribute::Else;
                }
            } else if prev.removed {
                *attr = Attribute::If(expr.clone()); // previous node was deleted, make this an if
            }
        } else if let Attribute::Else = attr {
            if prev.is_true {
                remove_node = true; // previous node was chosen, delete the whole node
            } else if prev.removed {
                remove_attr = true; // previous node was deleted, delete this attribute
            }
        } else {
            // we keep non-condcomp attributes
            return true;
        }

        !remove_attr
    });

    prev.is_true = is_true || prev.is_true;
    prev.removed = remove_node;
    Ok(())
}

fn eval_opt_attr(
    opt_node: &mut Option<impl SyntaxNode>,
    prev: &mut PrevEval,
    features: &Features,
) -> Result<(), E> {
    if let Some(node) = opt_node {
        eval_if_attr(node, prev, features)?;
        if prev.removed {
            *opt_node = None;
        }
    }
    Ok(())
}

fn eval_if_attrs(nodes: &mut Vec<impl SyntaxNode>, features: &Features) -> Result<PrevEval, E> {
    let mut prev = PrevEval {
        has_if: false,
        is_true: false,
        removed: false,
    };
    let mut err = None;

    // remove the nodes for which the attr evaluate to false.
    nodes.retain_mut(|node| {
        let res = eval_if_attr(node, &mut prev, features);
        if let (Err(e), None) = (res, &err) {
            err = Some(e);
        }
        !prev.removed // keep the node if attr is unresolved or true.
    });

    if let Some(e) = err {
        Err(e)
    } else {
        Ok(prev)
    }
}

fn stmt_eval_if_attrs(statements: &mut Vec<StatementNode>, features: &Features) -> Result<(), E> {
    fn rec_one(stmt: &mut StatementNode, feats: &Features) -> Result<(), E> {
        match stmt.node_mut() {
            Statement::Compound(stmt) => {
                rec(&mut stmt.statements, feats)?;
            }
            Statement::If(stmt) => {
                rec(&mut stmt.if_clause.body.statements, feats)?;
                for elif in &mut stmt.else_if_clauses {
                    rec(&mut elif.body.statements, feats)?;
                }
                if let Some(el) = &mut stmt.else_clause {
                    rec(&mut el.body.statements, feats)?;
                }
            }
            Statement::Switch(stmt) => {
                eval_if_attrs(&mut stmt.clauses, feats)?;
                for clause in &mut stmt.clauses {
                    rec(&mut clause.body.statements, feats)?;
                }
            }
            Statement::Loop(stmt) => {
                let mut prev = rec(&mut stmt.body.statements, feats)?;
                eval_opt_attr(&mut stmt.continuing, &mut prev, feats)?;
                if let Some(cont) = &mut stmt.continuing {
                    rec(&mut cont.body.statements, feats)?;
                    eval_opt_attr(&mut cont.break_if, &mut prev, feats)?;
                }
                rec(&mut stmt.body.statements, feats)?;
            }
            Statement::For(stmt) => {
                if let Some(init) = &mut stmt.initializer {
                    rec_one(&mut *init, feats)?
                }
                if let Some(updt) = &mut stmt.update {
                    rec_one(&mut *updt, feats)?
                }
                rec(&mut stmt.body.statements, feats)?;
            }
            Statement::While(stmt) => {
                rec(&mut stmt.body.statements, feats)?;
            }
            _ => (),
        };
        Ok(())
    }
    fn rec(stats: &mut Vec<StatementNode>, feats: &Features) -> Result<PrevEval, E> {
        let prev = eval_if_attrs(stats, feats)?;
        for stmt in stats {
            rec_one(stmt, feats)?;
        }
        Ok(prev)
    }
    rec(statements, features).map(|_| ())
}

/// Run the conditional translation phase, eliminating unused code branches.
pub fn condcomp(module: &mut TranslationUnit, features: &Features) -> Result<(), E> {
    module.remove_voids();
    eval_if_attrs(&mut module.imports, features)?;
    eval_if_attrs(&mut module.global_directives, features)?;
    eval_if_attrs(&mut module.global_declarations, features)?;

    for decl in &mut module.global_declarations {
        if let GlobalDeclaration::Struct(decl) = decl.node_mut() {
            eval_if_attrs(&mut decl.members, features)
                .map_err(|e| Diagnostic::from(e).with_declaration(decl.ident.to_string()))?;
        } else if let GlobalDeclaration::Function(decl) = decl.node_mut() {
            eval_if_attrs(&mut decl.parameters, features)
                .map_err(|e| Diagnostic::from(e).with_declaration(decl.ident.to_string()))?;
            stmt_eval_if_attrs(&mut decl.body.statements, features)
                .map_err(|e| Diagnostic::from(e).with_declaration(decl.ident.to_string()))?;
        }
    }

    Ok(())
}
