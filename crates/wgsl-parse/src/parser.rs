use std::str::FromStr;

use crate::{
    error::Error,
    lexer::{Lexer, Token, TokenIterator},
    syntax::*,
};

#[allow(clippy::all, reason = "generated code")]
mod grammar {
    include!("grammar.rs");
}

pub use crate::parser_support::ParseEntryPoint;
use grammar::EntryPointParser;

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
impl FromStr for crate::syntax::ImportStatement {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse!(source, EntryPointImportStatement, ImportStatement)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn expect_err<T: FromStr + std::fmt::Display>(expr: &str)
    where
        <T as FromStr>::Err: std::fmt::Debug,
    {
        let parsed = T::from_str(expr);
        assert!(
            parsed.is_err(),
            "case `{expr}`: expected Err, got Ok({})",
            parsed.unwrap()
        );
    }

    fn expect_ok<T: FromStr + std::fmt::Debug>(expr: &str)
    where
        <T as FromStr>::Err: std::fmt::Display,
    {
        let parsed = T::from_str(expr);
        assert!(
            parsed.is_ok(),
            "case `{expr}`: expected Ok, got Err({})",
            parsed.unwrap_err()
        );
    }

    #[test]
    fn operator_prec_assoc() {
        // failure cases from the WGSL spec
        expect_ok::<Expression>("x & (y ^ (z | w))"); // Invalid: x & y ^ z | w
        expect_ok::<Expression>("(x + y) << (z >= w)"); // Invalid: x + y << z >= w
        expect_ok::<Expression>("x < (y > z)"); // Invalid: x < y > z
        expect_ok::<Expression>("x && (y || z)"); // Invalid: x && y || z
        expect_err::<Expression>("x & y ^ z | w");
        expect_err::<Expression>("x + y << z >= w");
        expect_err::<Expression>("x < y > z");
        expect_err::<Expression>("x && y || z");
        // more cases
        expect_ok::<Expression>("x && y && z");
        expect_ok::<Expression>("x || y || z");
        expect_ok::<Expression>("x & y & z");
        expect_ok::<Expression>("x ^ y ^ z");
        expect_ok::<Expression>("x | y | z");
        expect_err::<Expression>("x >> y >> z");
        expect_err::<Expression>("x << y << z");
        expect_ok::<Expression>("x && y < z && &w");
        expect_ok::<Expression>("x & -y & &z");
    }

    #[test]
    fn if_statement_else_clauses_have_no_attributes() {
        use crate::SyntaxNode;
        use crate::syntax::{ElseClause, ElseIfClause, Statement};
        use std::str::FromStr;

        let src = "if (a) {} else if (b) {} else {}";
        let stmt = Statement::from_str(src).expect("parse if with else branches");
        let Statement::If(if_stmt) = stmt else {
            panic!("expected if statement");
        };

        assert_eq!(if_stmt.else_if_clauses.len(), 1);
        assert!(if_stmt.else_clause.is_some());

        let elif = &if_stmt.else_if_clauses[0];
        assert!(elif.attributes().is_empty());
        let ElseIfClause {
            expression,
            body: _,
        } = elif;
        assert_eq!(expression.to_string(), "(b)");

        let else_clause = if_stmt.else_clause.as_ref().unwrap();
        assert!(else_clause.attributes().is_empty());
        let ElseClause { body: _ } = else_clause;

        assert!(!if_stmt.to_string().contains('@'));
    }

    #[test]
    fn lhs_expression() {
        expect_ok::<Statement>("(x) = 1;");
        expect_ok::<Statement>("(*x) += 1;");
        expect_ok::<Statement>("&(*x) += 1;");
        expect_ok::<Statement>("x[0] = 1;");
        expect_ok::<Statement>("x.y[0] = 1;");
        expect_ok::<Statement>("x[0].y = 1;");

        // WESL extensions
        expect_ok::<Statement>("@if(true) x = 1;");
        expect_ok::<Statement>("@if(true) &(*x) += 1;");
        expect_err::<Statement>("@if(true) (*x) += 1;");
        expect_ok::<Statement>("x::y = 1;");
    }
}
