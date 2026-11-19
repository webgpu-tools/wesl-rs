#![doc = include_str!("../README.md")]

#[cfg(test)]
mod tests;

use std::fmt::{Display, Formatter};
pub use wesl_quote_impl::{quote_expression, quote_literal, quote_module, quote_statement};
use wgsl_parse::{
    span::Spanned,
    syntax::{
        AssignmentStatement, Attribute, BinaryExpression, BreakStatement, CompoundStatement,
        ConstAssert, ContinueStatement, Declaration, DecrementStatement, DiagnosticDirective,
        DiscardStatement, EnableDirective, Expression, ForStatement, Function,
        FunctionCallExpression, FunctionCallStatement, GlobalDeclaration, GlobalDirective, Ident,
        IfStatement, ImportStatement, IncrementStatement, IndexingExpression, LiteralExpression,
        LoopStatement, NamedComponentExpression, ParenthesizedExpression, RequiresDirective,
        ReturnStatement, Statement, Struct, StructMember, SwitchStatement, TypeAlias,
        TypeExpression, UnaryExpression, WhileStatement,
    },
};

/// A type for using WGSL types in wgsl-quote.
pub struct QuoteWgsl<'a, T: ?Sized>(pub &'a T);

macro_rules! DisplayFor {
    (type $t:ty) => {
        impl Display for QuoteWgsl<'_, $t> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Display::fmt(&self.0, f)
            }
        }
    };
    (type $t:ty, $variant:expr) => {
        impl Display for QuoteWgsl<'_, $t> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Display::fmt(&$variant(*self.0), f)
            }
        }
    };
}

DisplayFor!(type ImportStatement);

DisplayFor!(type GlobalDirective);
DisplayFor!(type DiagnosticDirective);
DisplayFor!(type EnableDirective);
DisplayFor!(type RequiresDirective);

DisplayFor!(type GlobalDeclaration);
DisplayFor!(type Spanned<GlobalDeclaration>);
DisplayFor!(type Declaration);
DisplayFor!(type TypeAlias);
DisplayFor!(type Struct);
DisplayFor!(type Function);
DisplayFor!(type ConstAssert);

DisplayFor!(type Attribute);
DisplayFor!(type Spanned<Attribute>);

DisplayFor!(type StructMember);
DisplayFor!(type Spanned<StructMember>);

DisplayFor!(type Statement);
DisplayFor!(type Spanned<Statement>);
DisplayFor!(type CompoundStatement);
DisplayFor!(type AssignmentStatement);
DisplayFor!(type IncrementStatement);
DisplayFor!(type DecrementStatement);
DisplayFor!(type IfStatement);
DisplayFor!(type SwitchStatement);
DisplayFor!(type LoopStatement);
DisplayFor!(type ForStatement);
DisplayFor!(type WhileStatement);
DisplayFor!(type BreakStatement);
DisplayFor!(type ContinueStatement);
DisplayFor!(type ReturnStatement);
DisplayFor!(type DiscardStatement);
DisplayFor!(type FunctionCallStatement);
// DisplayFor!(type ConstAssertStatement);
// DisplayFor!(type DeclarationStatement);

DisplayFor!(type Expression);
DisplayFor!(type Spanned<Expression>);
DisplayFor!(type LiteralExpression);
DisplayFor!(type ParenthesizedExpression);
DisplayFor!(type NamedComponentExpression);
DisplayFor!(type IndexingExpression);
DisplayFor!(type UnaryExpression);
DisplayFor!(type BinaryExpression);
DisplayFor!(type FunctionCallExpression);
DisplayFor!(type TypeExpression);

DisplayFor!(type bool, LiteralExpression::Bool);
DisplayFor!(type i32, LiteralExpression::I32);
DisplayFor!(type u32, LiteralExpression::U32);
DisplayFor!(type f32, LiteralExpression::F32);

impl Display for QuoteWgsl<'_, Ident> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
