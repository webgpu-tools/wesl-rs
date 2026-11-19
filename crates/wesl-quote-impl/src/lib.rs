mod quote_macro;

use proc_macro_error3::proc_macro_error;
use quote_macro::{QuoteNodeKind, quote_impl};

#[proc_macro_error]
#[proc_macro]
pub fn quote_module(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    quote_impl(QuoteNodeKind::TranslationUnit, input.into()).into()
}
#[proc_macro_error]
#[proc_macro]
pub fn quote_literal(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    quote_impl(QuoteNodeKind::Literal, input.into()).into()
}
#[proc_macro_error]
#[proc_macro]
pub fn quote_expression(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    quote_impl(QuoteNodeKind::Expression, input.into()).into()
}
#[proc_macro_error]
#[proc_macro]
pub fn quote_statement(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    quote_impl(QuoteNodeKind::Statement, input.into()).into()
}
