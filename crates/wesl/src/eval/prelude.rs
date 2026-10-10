use std::sync::LazyLock;

use wgsl_parse::{parse_str, syntax::*};

pub static EXPR_TRUE: Expression = Expression::Literal(LiteralExpression::Bool(true));
pub static EXPR_FALSE: Expression = Expression::Literal(LiteralExpression::Bool(false));
pub static ATTR_INTRINSIC: LazyLock<Attribute> = LazyLock::new(|| {
    Attribute::Custom(CustomAttribute {
        name: "__intrinsic".to_string(),
        arguments: None,
    })
});

pub static PRELUDE: LazyLock<TranslationUnit> = LazyLock::new(|| {
    let mut module = parse_str(include_str!("./prelude.wesl")).unwrap();
    crate::pass::retarget_idents(&mut module);
    module
});
