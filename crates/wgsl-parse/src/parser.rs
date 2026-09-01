use std::str::FromStr;

use crate::{
    error::Error,
    lexer::{Lexer, Token, TokenIterator},
    syntax::*,
};

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(
    #[allow(clippy::all, reason = "generated code")]
    wgsl
);

pub use crate::parser_support::ParseEntryPoint;
use wgsl::EntryPointParser;

macro_rules! parse {
    ($source:expr, $token:ident, $entrypoint:ident) => {{
        match parse_tokens(Lexer::new($source), Token::$token) {
            Ok(ParseEntryPoint::$entrypoint(res)) => Ok(res),
            Ok(_) => unreachable!("parser parsed the wrong entrypoint"),
            Err(e) => Err(e),
        }
    }};
}

/// Parse a token stream into a syntax tree.
///
/// Low-level implementation, you probably don't want to use this. See [`parse_str`].
///
/// The `entrypoint` parameter must be one of the `EntryPointXXX` tokens, which tells the
/// parser which syntax node to expect. It returns the [`ParseEntryPoint`] union type.
pub fn parse_tokens(
    lexer: impl TokenIterator,
    entrypoint: Token,
) -> Result<ParseEntryPoint, Error> {
    let lexer = std::iter::once(Ok((0, entrypoint, 0))).chain(lexer);
    let parser = EntryPointParser::new();
    parser.parse(lexer).map_err(Into::into)
}

/// Parse a string into a syntax tree ([`TranslationUnit`]).
///
/// Identical to [`TranslationUnit::from_str`].
pub fn parse_str(source: &str) -> Result<TranslationUnit, Error> {
    parse!(source, EntryPointTranslationUnit, TranslationUnit)
}

pub fn recognize_template_list(lexer: impl TokenIterator) -> Result<(), Error> {
    match parse_tokens(lexer, Token::EntryPointTryTemplateList) {
        Ok(ParseEntryPoint::TryTemplateList(_)) => Ok(()),
        Ok(_) => unreachable!("parser parsed the wrong entrypoint"),
        Err(e) => Err(e),
    }
}

impl FromStr for TranslationUnit {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointTranslationUnit, TranslationUnit)
    }
}
impl FromStr for GlobalDirective {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointGlobalDirective, GlobalDirective)
    }
}
impl FromStr for GlobalDeclaration {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointGlobalDecl, GlobalDecl)
    }
}
impl FromStr for Statement {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointStatement, Statement)
    }
}
impl FromStr for Expression {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointExpression, Expression)
    }
}
impl FromStr for LiteralExpression {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointLiteral, Literal)
    }
}
#[cfg(feature = "imports")]
impl FromStr for crate::syntax::ImportStatement {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointImportStatement, ImportStatement)
    }
}

#[test]
fn test_operator_prec_assoc() {
    fn expect_expr_err(expr: &str) {
        let parsed = Expression::from_str(expr);
        assert!(
            parsed.is_err(),
            "case `{expr}`: expected Err, got Ok({})",
            parsed.unwrap()
        );
    }

    fn expect_expr_ok(expr: &str) {
        let parsed = Expression::from_str(expr);
        assert!(
            parsed.is_ok(),
            "case `{expr}`: expected Ok, got Err({})",
            parsed.unwrap_err()
        );
    }

    // failure cases from the WGSL spec
    expect_expr_ok("x & (y ^ (z | w))"); // Invalid: x & y ^ z | w
    expect_expr_ok("(x + y) << (z >= w)"); // Invalid: x + y << z >= w
    expect_expr_ok("x < (y > z)"); // Invalid: x < y > z
    expect_expr_ok("x && (y || z)"); // Invalid: x && y || z
    expect_expr_err("x & y ^ z | w");
    expect_expr_err("x + y << z >= w");
    expect_expr_err("x < y > z");
    expect_expr_err("x && y || z");
    // more cases
    expect_expr_ok("x && y && z");
    expect_expr_ok("x || y || z");
    expect_expr_ok("x & y & z");
    expect_expr_ok("x ^ y ^ z");
    expect_expr_ok("x | y | z");
    expect_expr_err("x >> y >> z");
    expect_expr_err("x << y << z");
    expect_expr_ok("x && y < z && &w");
    expect_expr_ok("x & -y & &z");
}
