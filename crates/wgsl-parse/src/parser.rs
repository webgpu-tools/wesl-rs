use std::str::FromStr;

use crate::{
    error::Error,
    lexer::{Lexer, TokenIterator},
    syntax::{Expression, GlobalDeclaration, GlobalDirective, Statement, TranslationUnit},
};

mod wgsl {
    include!("wgsl.rs");
}

#[cfg(feature = "wesl")]
pub use wgsl::ImportStatementParser;

pub use wgsl::{
    ExpressionParser, GlobalDeclParser, GlobalDirectiveParser, LiteralParser, StatementParser,
    TranslationUnitParser, TryTemplateListParser,
};

/// Parse a string into a syntax tree ([`TranslationUnit`]).
///
/// Identical to [`TranslationUnit::from_str`].
pub fn parse_str(source: &str) -> Result<TranslationUnit, Error> {
    let lexer = Lexer::new(source);
    let parser = TranslationUnitParser::new();
    parser.parse(lexer).map_err(Into::into)
}

pub fn recognize_template_list(lexer: impl TokenIterator) -> Result<(), Error> {
    let parser = TryTemplateListParser::new();
    parser.parse(lexer).map(|_| ()).map_err(Into::into)
}

impl FromStr for TranslationUnit {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let lexer = Lexer::new(source);
        let parser = TranslationUnitParser::new();
        parser.parse(lexer).map_err(Into::into)
    }
}
impl FromStr for GlobalDirective {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let lexer = Lexer::new(source);
        let parser = GlobalDirectiveParser::new();
        parser.parse(lexer).map_err(Into::into)
    }
}
impl FromStr for GlobalDeclaration {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let lexer = Lexer::new(source);
        let parser = GlobalDeclParser::new();
        parser.parse(lexer).map_err(Into::into)
    }
}
impl FromStr for Statement {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let lexer = Lexer::new(source);
        let parser = StatementParser::new();
        parser.parse(lexer).map_err(Into::into)
    }
}
impl FromStr for Expression {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let lexer = Lexer::new(source);
        let parser = ExpressionParser::new();
        parser.parse(lexer).map_err(Into::into)
    }
}
#[cfg(feature = "wesl")]
impl FromStr for crate::syntax::ImportStatement {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let lexer = Lexer::new(source);
        let parser = ImportStatementParser::new();
        parser.parse(lexer).map_err(Into::into)
    }
}
