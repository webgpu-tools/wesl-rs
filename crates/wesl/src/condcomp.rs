use std::collections::HashMap;

use crate::Diagnostic;
use thiserror::Error;
use wgsl_parse::{SyntaxNode, span::Spanned, syntax::*};

/// Conditional translation error.
#[derive(Clone, Debug, Error)]
pub enum CondCompError {
    #[error("invalid feature flag: `{0}`")]
    InvalidFeatureFlag(String),
    #[error("unexpected feature flag: `{0}`")]
    UnexpectedFeatureFlag(String),
    #[error("invalid if attribute expression: `{0}`")]
    InvalidExpression(Expression),
    #[error("an @elif or @else attribute must be preceded by a @if or @elif on the previous node")]
    NoPrecedingIf,
    #[error("cannot have multiple @if/@elif/@else attributes on the same node")]
    DuplicateIf,
}

type E = crate::Error;

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

/// Conditional state of a syntax node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeEval {
    /// Whether the syntax node has an if, elif or else attribute.
    has_condcomp: bool,
    /// Whether the node evaluates to false and is removed.
    /// e.g. `@if(false)` or `@if(true) ... @else`
    is_false: bool,
    /// Whether this node, or a previous node in the if/elif chain, evaluates to true.
    /// All following elif/else nodes in the chain therefore evaluate to false.
    chain_has_true: bool,
}

/// * ensure there is at most one if/elif/else node.
/// * ensure elif/else nodes are preceded by if/elif.
/// * remove the attributes which evaluate to true.
/// * turn elifs into ifs when previous node was deleted.
/// * turn elifs into elses when it evaluates to true.
fn eval_if_attr(
    node: &mut impl SyntaxNode,
    prev: &mut NodeEval,
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
    prev: &mut NodeEval,
    features: &Features,
) -> Result<(), E> {
    let attr = get_single_attr(node.attributes_mut())?;
    if let Some(attr) = attr {
        prev.has_condcomp = attr.is_condcomp();
        if let Attribute::If(expr) = attr.node_mut() {
            **expr = eval_attr(expr, features)?;
            // a new `if` starts a new if/elif/else chain
            prev.chain_has_true = false;
        } else if let Attribute::Elif(expr) = attr.node_mut() {
            if !prev.has_condcomp {
                return Err(CondCompError::NoPrecedingIf.into());
            } else {
                **expr = eval_attr(expr, features)?;
            }
        } else if let Attribute::Else = attr.node()
            && !prev.has_condcomp
        {
            return Err(CondCompError::NoPrecedingIf.into());
        }
    } else {
        prev.has_condcomp = false;
    }

    let mut is_false = false;

    node.retain_attributes_mut(|attr| {
        let mut remove_attr = false;
        if let Attribute::If(expr) = attr {
            if **expr == EXPR_TRUE {
                remove_attr = true;
                prev.chain_has_true = true;
            } else if **expr == EXPR_FALSE {
                is_false = true;
            }
        } else if let Attribute::Elif(expr) = attr {
            if prev.chain_has_true || **expr == EXPR_FALSE {
                is_false = true; // a previous node was chosen, delete the whole node
            } else if **expr == EXPR_TRUE {
                if prev.is_false {
                    remove_attr = true; // the previous node is false and is deleted, so elif(true) can be removed
                } else {
                    *attr = Attribute::Else; // the previous node is undecided, but elif(true) can become else.
                }
                prev.chain_has_true = true;
            } else if prev.is_false {
                *attr = Attribute::If(expr.clone()); // the previous node is false and is deleted, elif becomes if
            }
        } else if let Attribute::Else = attr {
            if prev.chain_has_true {
                is_false = true; // a previous node was chosen, delete the whole node
            } else if prev.is_false {
                remove_attr = true; // the previous node was deleted, delete this attribute
                prev.chain_has_true = true;
            }
        } else {
            // we keep non-condcomp attributes
            return true;
        }

        !remove_attr
    });

    prev.is_false = is_false;
    Ok(())
}

fn eval_opt_attr(
    opt_node: &mut Option<impl SyntaxNode>,
    prev: &mut NodeEval,
    features: &Features,
) -> Result<(), E> {
    if let Some(node) = opt_node {
        eval_if_attr(node, prev, features)?;
        if prev.chain_has_true && !prev.is_false {
            *opt_node = None;
        }
    }
    Ok(())
}

fn eval_if_attrs(nodes: &mut Vec<impl SyntaxNode>, features: &Features) -> Result<NodeEval, E> {
    let mut prev = NodeEval {
        has_condcomp: false,
        chain_has_true: false,
        is_false: false,
    };
    let mut err = None;

    // remove the nodes for which the attr evaluate to false.
    nodes.retain_mut(|node| {
        let res = eval_if_attr(node, &mut prev, features);
        if let (Err(e), None) = (res, &err) {
            err = Some(e);
        }
        !prev.is_false
    });

    if let Some(e) = err {
        Err(e)
    } else {
        Ok(prev)
    }
}

fn stmt_eval_if_attrs(statements: &mut Vec<StatementNode>, features: &Features) -> Result<(), E> {
    fn rec_eval_inside_stmt(stmt: &mut StatementNode, feats: &Features) -> Result<(), E> {
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
                    rec_eval_inside_stmt(&mut *init, feats)?
                }
                if let Some(updt) = &mut stmt.update {
                    rec_eval_inside_stmt(&mut *updt, feats)?
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

    fn rec(stmts: &mut Vec<StatementNode>, feats: &Features) -> Result<NodeEval, E> {
        let mut prev = NodeEval {
            has_condcomp: false,
            chain_has_true: false,
            is_false: false,
        };

        // If an `@if` decorates a compound statement, the statement gets flattened.
        // This is the same code as in eval_if_attrs, except it flattens the compound when it evaluates to true.
        {
            let mut i = 0;

            // remove the nodes for which the attr evaluate to false.
            while let Some(node) = stmts.get_mut(i) {
                eval_if_attr(node, &mut prev, feats)?;
                if let Statement::Compound(stmt) = &**node
                    && prev.has_condcomp
                    && !prev.is_false
                {
                    // replace the compound statements with its contents
                    // TODO: other compound statement attributes are lost. validation has no opportunity to check them.
                    // COMBAK: this clone is unnecessary and probably inefficient.
                    let mut body = stmt.statements.clone();
                    rec(&mut body, feats)?;
                    let n = body.len();
                    stmts.splice(i..i + 1, body);
                    i += n;
                } else if prev.is_false {
                    stmts.remove(i);
                } else {
                    rec_eval_inside_stmt(node, feats)?;
                    i += 1;
                }
            }
        }

        Ok(prev)
    }

    rec(statements, features).map(|_| ())
}

pub fn run(wesl: &mut TranslationUnit, features: &Features) -> Result<(), E> {
    wesl.remove_voids();
    eval_if_attrs(&mut wesl.imports, features)?;
    eval_if_attrs(&mut wesl.global_directives, features)?;

    // If an `@if` decorates a compound global declaration, it gets flattened.
    // This is the same code as in eval_if_attrs, except it flattens the compound when it evaluates to true.
    fn eval_flatten_compound(
        decls: &mut Vec<GlobalDeclarationNode>,
        features: &Features,
    ) -> Result<(), E> {
        let mut prev = NodeEval {
            has_condcomp: false,
            chain_has_true: false,
            is_false: false,
        };

        let mut i = 0;

        // remove the nodes for which the attr evaluate to false.
        while let Some(node) = decls.get_mut(i) {
            eval_if_attr(node, &mut prev, features)?;
            if let GlobalDeclaration::Compound(stmt) = &**node
                && prev.has_condcomp
                && !prev.is_false
            {
                // replace the compound statements with its contents
                // TODO: other compound statement attributes are lost. validation has no opportunity to check them.
                // COMBAK: this clone is unnecessary and probably inefficient.
                let mut body = stmt.body.clone();
                eval_flatten_compound(&mut body, features)?;
                let n = body.len();
                decls.splice(i..i + 1, body);
                i += n;
            } else if prev.is_false {
                decls.remove(i);
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    eval_flatten_compound(&mut wesl.global_declarations, features)?;

    for decl in &mut wesl.global_declarations {
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
