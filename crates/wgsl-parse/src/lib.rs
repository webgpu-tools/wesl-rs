#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod syntax;

mod parser_support;
mod syntax_impl;
mod syntax_pretty;
mod syntax_spans;

#[cfg(feature = "tokrepr")]
mod tokrepr;
#[cfg(feature = "tokrepr")]
pub use ::tokrepr::TokRepr;

pub use error::Error;
pub use parser::parse_str;
pub use syntax_impl::SyntaxNode;
pub use syntax_pretty::{Alloc, Doc, ToDoc};
pub use syntax_spans::{PrintedSpan, print_with_spans};
