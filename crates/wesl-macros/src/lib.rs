//! macros for [wesl].
//!
//! [wesl]: https://docs.rs/wesl
#![cfg(feature = "query")]

mod query_macro;

use query_macro::{QueryInput, query_impl};

#[proc_macro]
pub fn query(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as QueryInput);
    query_impl(input, false).into()
}
#[proc_macro]
pub fn query_mut(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as QueryInput);
    query_impl(input, true).into()
}
