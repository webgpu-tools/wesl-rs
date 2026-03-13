//! support functions to be injected in the lalrpop parser.

use std::str::FromStr;

use itertools::Itertools;

use crate::{
    error::ParseError,
    span::{Span, Spanned},
    syntax::*,
};

type E = ParseError;

pub(crate) enum Component {
    Named(Ident),
    Index(ExpressionNode),
}

pub(crate) fn apply_components(
    expr: Expression,
    span: Span,
    components: Vec<Spanned<Component>>,
) -> Expression {
    components.into_iter().fold(expr, |base, comp| {
        let span = span.extend(comp.span());
        let base = Spanned::new(base, span);
        match comp.into_inner() {
            Component::Named(component) => {
                Expression::NamedComponent(NamedComponentExpression { base, component })
            }
            Component::Index(index) => Expression::Indexing(IndexingExpression { base, index }),
        }
    })
}

impl FromStr for DeclarationKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "const" => Ok(Self::Const),
            "override" => Ok(Self::Override),
            "let" => Ok(Self::Let),
            "var" => Ok(Self::Var(None)),
            _ => Err(()),
        }
    }
}

fn one_arg(arguments: Option<Vec<ExpressionNode>>) -> Option<ExpressionNode> {
    match arguments {
        Some(mut args) => (args.len() == 1).then(|| args.pop().unwrap()),
        None => None,
    }
}
fn two_args(arguments: Option<Vec<ExpressionNode>>) -> Option<(ExpressionNode, ExpressionNode)> {
    match arguments {
        Some(args) => (args.len() == 2).then(|| args.into_iter().collect_tuple().unwrap()),
        None => None,
    }
}
fn zero_args(arguments: Option<Vec<ExpressionNode>>) -> bool {
    arguments.is_none()
}
fn ident(expr: ExpressionNode) -> Option<Ident> {
    match expr.into_inner() {
        Expression::TypeOrIdentifier(TypeExpression {
            #[cfg(feature = "imports")]
                path: _,
            ident,
            template_args: None,
        }) => Some(ident),
        _ => None,
    }
}

pub(crate) fn parse_attribute(
    name: String,
    args: Option<Vec<ExpressionNode>>,
) -> Result<Attribute, E> {
    match name.as_str() {
        "align" => match one_arg(args) {
            Some(expr) => Ok(Attribute::Align(expr)),
            _ => Err(E::Attribute("align", "expected 1 argument")),
        },
        "binding" => match one_arg(args) {
            Some(expr) => Ok(Attribute::Binding(expr)),
            _ => Err(E::Attribute("binding", "expected 1 argument")),
        },
        "blend_src" => match one_arg(args) {
            Some(expr) => Ok(Attribute::BlendSrc(expr)),
            _ => Err(E::Attribute("blend_src", "expected 1 argument")),
        },
        "builtin" => match one_arg(args) {
            Some(expr) => match ident(expr).and_then(|id| id.name().parse().ok()) {
                Some(b) => Ok(Attribute::Builtin(b)),
                _ => Err(E::Attribute(
                    "builtin",
                    "the argument is not a valid built-in value name",
                )),
            },
            _ => Err(E::Attribute("builtin", "expected 1 argument")),
        },
        "const" => match zero_args(args) {
            true => Ok(Attribute::Const),
            false => Err(E::Attribute("const", "expected 0 arguments")),
        },
        "diagnostic" => match two_args(args) {
            Some((e1, e2)) => {
                let severity = ident(e1).and_then(|id| id.name().parse().ok());
                let rule = match e2.into_inner() {
                    Expression::TypeOrIdentifier(TypeExpression {
                        #[cfg(feature = "imports")]
                            path: _,
                        ident,
                        template_args: None,
                    }) => Some(ident.name().to_string()),
                    Expression::NamedComponent(e) => {
                        ident(e.base).map(|id| format!("{}.{}", id.name(), e.component))
                    }
                    _ => None,
                };
                match (severity, rule) {
                    (Some(severity), Some(rule)) => {
                        Ok(Attribute::Diagnostic(DiagnosticAttribute {
                            severity,
                            rule,
                        }))
                    }
                    _ => Err(E::Attribute("diagnostic", "invalid arguments")),
                }
            }
            _ => Err(E::Attribute("diagnostic", "expected 1 argument")),
        },
        "group" => match one_arg(args) {
            Some(expr) => Ok(Attribute::Group(expr)),
            _ => Err(E::Attribute("group", "expected 1 argument")),
        },
        "id" => match one_arg(args) {
            Some(expr) => Ok(Attribute::Id(expr)),
            _ => Err(E::Attribute("id", "expected 1 argument")),
        },
        "interpolate" => match args {
            Some(v) if v.len() == 2 => {
                let (e1, e2) = v.into_iter().collect_tuple().unwrap();
                let ty = ident(e1).and_then(|id| id.name().parse().ok());
                let sampling = ident(e2).and_then(|id| id.name().parse().ok());
                match (ty, sampling) {
                    (Some(ty), Some(sampling)) => {
                        Ok(Attribute::Interpolate(InterpolateAttribute {
                            ty,
                            sampling: Some(sampling),
                        }))
                    }
                    _ => Err(E::Attribute("interpolate", "invalid arguments")),
                }
            }
            Some(v) if v.len() == 1 => {
                let e1 = v.into_iter().next().unwrap();
                let ty = ident(e1).and_then(|id| id.name().parse().ok());
                match ty {
                    Some(ty) => Ok(Attribute::Interpolate(InterpolateAttribute {
                        ty,
                        sampling: None,
                    })),
                    _ => Err(E::Attribute("interpolate", "invalid arguments")),
                }
            }
            _ => Err(E::Attribute("interpolate", "invalid arguments")),
        },

        "invariant" => match zero_args(args) {
            true => Ok(Attribute::Invariant),
            false => Err(E::Attribute("invariant", "expected 0 arguments")),
        },
        "location" => match one_arg(args) {
            Some(expr) => Ok(Attribute::Location(expr)),
            _ => Err(E::Attribute("location", "expected 1 argument")),
        },
        "must_use" => match zero_args(args) {
            true => Ok(Attribute::MustUse),
            false => Err(E::Attribute("must_use", "expected 0 arguments")),
        },
        "size" => match one_arg(args) {
            Some(expr) => Ok(Attribute::Size(expr)),
            _ => Err(E::Attribute("size", "expected 1 argument")),
        },
        "workgroup_size" => match args {
            Some(args) => {
                let mut it = args.into_iter();
                match (it.next(), it.next(), it.next(), it.next()) {
                    (Some(x), y, z, None) => {
                        Ok(Attribute::WorkgroupSize(WorkgroupSizeAttribute { x, y, z }))
                    }
                    _ => Err(E::Attribute("workgroup_size", "expected 1-3 arguments")),
                }
            }
            _ => Err(E::Attribute("workgroup_size", "expected 1-3 arguments")),
        },
        "vertex" => match zero_args(args) {
            true => Ok(Attribute::Vertex),
            false => Err(E::Attribute("vertex", "expected 0 arguments")),
        },
        "fragment" => match zero_args(args) {
            true => Ok(Attribute::Fragment),
            false => Err(E::Attribute("fragment", "expected 0 arguments")),
        },
        "compute" => match zero_args(args) {
            true => Ok(Attribute::Compute),
            false => Err(E::Attribute("compute", "expected 0 arguments")),
        },
        #[cfg(feature = "imports")]
        "publish" => Ok(Attribute::Publish),
        #[cfg(feature = "condcomp")]
        "if" => match one_arg(args) {
            Some(expr) => Ok(Attribute::If(expr)),
            None => Err(E::Attribute("if", "expected 1 argument")),
        },
        #[cfg(feature = "condcomp")]
        "elif" => match one_arg(args) {
            Some(expr) => Ok(Attribute::Elif(expr)),
            None => Err(E::Attribute("elif", "expected 1 argument")),
        },
        #[cfg(feature = "condcomp")]
        "else" => match zero_args(args) {
            true => Ok(Attribute::Else),
            false => Err(E::Attribute("else", "expected 0 arguments")),
        },
        #[cfg(feature = "generics")]
        "type" => parse_attr_type(args).map(Attribute::Type),
        #[cfg(feature = "naga-ext")]
        "early_depth_test" => match args {
            Some(args) => {
                let mut it = args.into_iter();
                match (it.next(), it.next()) {
                    (Some(expr), None) => match ident(expr).and_then(|id| id.name().parse().ok()) {
                        Some(c) => Ok(Attribute::EarlyDepthTest(Some(c))),
                        _ => Err(E::Attribute(
                            "early_depth_test",
                            "the argument must be one of `greater_equal`, `less_equal`, `unchanged`",
                        )),
                    },
                    (None, None) => Ok(Attribute::EarlyDepthTest(None)),
                    _ => Err(E::Attribute(
                        "early_depth_test",
                        "expected 0 or 1 arguments",
                    )),
                }
            }
            _ => Err(E::Attribute(
                "early_depth_test",
                "expected 0 or 1 arguments",
            )),
        },
        _ => Ok(Attribute::Custom(CustomAttribute {
            name,
            arguments: args,
        })),
    }
}

// format: @type(T, foo | bar | baz)
#[cfg(feature = "generics")]
fn parse_attr_type(arguments: Option<Vec<ExpressionNode>>) -> Result<TypeConstraint, E> {
    fn parse_rec(expr: Expression) -> Result<Vec<TypeExpression>, E> {
        match expr {
            Expression::TypeOrIdentifier(ty) => Ok(vec![ty]),
            Expression::Binary(BinaryExpression {
                operator: BinaryOperator::BitwiseOr,
                left,
                right,
            }) => {
                let ty = match right.into_inner() {
                    Expression::TypeOrIdentifier(ty) => Ok(ty),
                    _ => Err(E::Attribute(
                        "type",
                        "invalid second argument (type constraint)",
                    )),
                }?;
                let mut v = parse_rec(left.into_inner())?;
                v.push(ty);
                Ok(v)
            }
            _ => Err(E::Attribute(
                "type",
                "invalid second argument (type constraint)",
            )),
        }
    }
    match two_args(arguments) {
        Some((e1, e2)) => ident(e1)
            .map(|ident| {
                parse_rec(e2.into_inner()).map(|variants| TypeConstraint { ident, variants })
            })
            .unwrap_or_else(|| Err(E::Attribute("type", "invalid first argument (type name)"))),

        None => Err(E::Attribute("type", "expected 2 arguments")),
    }
}

pub(crate) fn parse_var_template(
    template_args: TemplateArgs,
) -> Result<Option<(AddressSpace, Option<AccessMode>)>, E> {
    match template_args {
        Some(tplt) => {
            let mut it = tplt.into_iter();
            match (it.next(), it.next(), it.next()) {
                (Some(e1), e2, None) => {
                    let addr_space = ident(e1.expression)
                        .and_then(|id| id.name().parse().ok())
                        .ok_or(E::VarTemplate("invalid address space"))?;
                    let mut access_mode = None;
                    if let Some(e2) = e2 {
                        if addr_space == AddressSpace::Storage {
                            access_mode = Some(
                                ident(e2.expression)
                                    .and_then(|id| id.name().parse().ok())
                                    .ok_or(E::VarTemplate("invalid access mode"))?,
                            );
                        } else {
                            return Err(E::VarTemplate(
                                "only variables with `storage` address space can have an access mode",
                            ));
                        }
                    }
                    Ok(Some((addr_space, access_mode)))
                }
                _ => Err(E::VarTemplate("template is empty")),
            }
        }
        None => Ok(None),
    }
}

macro_rules! lang_ext {
    ($feat:literal, $l:expr, $r:expr, $expr:expr) => {{
        #[cfg(feature = $feat)]
        {
            Ok($expr)
        }
        #[cfg(not(feature = $feat))]
        {
            Err(lalrpop_util::ParseError::User {
                error: ($l, ParseError::UnsupportedFeature($feat), $r),
            })
        }
    }};
}

pub(super) use lang_ext;
