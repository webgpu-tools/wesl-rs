use std::{iter::Peekable, str::FromStr};

use proc_macro_error3::abort;
use proc_macro2::{Literal, TokenStream};
use quote::ToTokens;
use token_stream_flatten::{FlattenRec, Token as RustToken};

/// Parse a part of WGSL/WESL syntax.
/// For global declarations and directives, just use the translation unit parser and pick out the pieces you need.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuoteNodeKind {
    TranslationUnit,
    Literal,
    Expression,
    Statement,
}

struct Lexer {
    token_stream: Peekable<FlattenRec>,
}

impl Lexer {
    fn new(input: TokenStream) -> Self {
        Self {
            token_stream: FlattenRec::from(input.into_iter()).peekable(),
        }
    }

    /// Expand a variable interpolation composed of a marker and a variable identifier (e.g. `#expr@ident`).
    fn marker2wesl(&mut self, span: proc_macro2::Span) -> LexerItem {
        let Some(RustToken::Ident(marker)) = self.token_stream.next() else {
            abort!(span, "expected an interpolation marker after `#`",)
        };
        let is_marker =
            matches!(self.token_stream.peek(), Some(RustToken::Punct(p)) if p.as_char() == '@');
        if !is_marker {
            // no marker (#ident) is equivalent to #expr@ident.
            return LexerItem::Interpolate(MarkerType::Expression, marker);
        }

        let _ = self.token_stream.next(); // consume the `@`.

        // consume the variable ident.
        let ident = match self.token_stream.next() {
            Some(RustToken::Ident(id)) => id,
            Some(tok) => abort!(tok.span(), "expected an identifier after `#{}@`", marker),
            None => abort!(span, "expected an identifier after `#{}@`", marker),
        };

        // syntactically valid placeholder
        // name clashes are allowed, since we do not check for semantic correctness
        match marker.to_string().as_str() {
            "expr" => LexerItem::Interpolate(MarkerType::Expression, ident),
            "decl" => LexerItem::Interpolate(MarkerType::Declaration, ident),
            "stmt" => LexerItem::Interpolate(MarkerType::Statement, ident),
            "mem" => LexerItem::Interpolate(MarkerType::StructMember, ident),
            "attr" => LexerItem::Interpolate(MarkerType::Attribute, ident),
            _ => abort!(span, "not a valid marker name"),
        }
    }
}

enum LexerItem {
    WgslCode(String, proc_macro2::Span),
    Interpolate(MarkerType, proc_macro2::Ident),
}

impl LexerItem {
    fn as_wgsl_code(&self) -> &str {
        match self {
            LexerItem::WgslCode(text, _) => text,
            LexerItem::Interpolate(MarkerType::Expression, _) => "dummy_expr",
            LexerItem::Interpolate(MarkerType::Declaration | MarkerType::Statement, _) => {
                "const dummy_name = dummy_expr;"
            }
            LexerItem::Interpolate(MarkerType::StructMember, _) => "dummy_name: dummy_type",
            LexerItem::Interpolate(MarkerType::Attribute, _) => "@dummy_attribute()",
        }
    }

    fn span(&self) -> proc_macro2::Span {
        match self {
            LexerItem::WgslCode(_, span) => *span,
            LexerItem::Interpolate(_, ident) => ident.span(),
        }
    }
}

enum MarkerType {
    Expression,
    Declaration,
    Statement,
    StructMember,
    Attribute,
}

impl Iterator for Lexer {
    type Item = LexerItem;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.token_stream.next()?;
        let span = token.span();
        let mut text = token.to_string();
        match token {
            RustToken::Delimiter(_) => Some(LexerItem::WgslCode(text, span)),
            RustToken::Ident(_) => {
                text.push(' ');
                Some(LexerItem::WgslCode(text, span))
            }
            RustToken::Literal(_) => {
                text.push(' ');
                Some(LexerItem::WgslCode(text, span))
            }
            RustToken::Punct(_) if text == "#" => Some(self.marker2wesl(span)),
            RustToken::Punct(punct) if punct.spacing() == proc_macro2::Spacing::Alone => {
                text.push(' ');
                Some(LexerItem::WgslCode(text, span))
            }
            RustToken::Punct(_) => Some(LexerItem::WgslCode(text, span)),
        }
    }
}

/// Our spans are reported in terms of where the generated WGSL has an error.
/// To map it back to tokens, we iterate over them until we find the corresponding range.
fn get_error_span(input: TokenStream, text_span: wgsl_parse::span::Span) -> proc_macro2::Span {
    Lexer::new(input)
        .scan((0, 0), |range, token| {
            range.0 = range.1;
            range.1 += token.as_wgsl_code().len();
            Some((*range, token))
        })
        .skip_while(|(range, _)| range.1 <= text_span.start)
        .take_while(|(range, _)| range.0 < text_span.end)
        .map(|(_, token)| token.span())
        .reduce(|base_span, extra_span| base_span.join(extra_span).unwrap_or(base_span))
        .unwrap_or_else(proc_macro2::Span::call_site)
}

fn to_wesl_tokens(input: TokenStream, ty: TokenStream) -> TokenStream {
    let mut format_arguments = Vec::new();
    let mut current_string = String::new();
    for token in Lexer::new(input.clone()) {
        match token {
            LexerItem::WgslCode(code, _) => {
                current_string.push_str(&code);
            }
            LexerItem::Interpolate(_, ident) => {
                if !current_string.is_empty() {
                    format_arguments
                        .push(proc_macro2::Literal::string(&current_string).to_token_stream());
                    current_string.clear();
                }
                format_arguments.push(quote::quote! { ::wesl_quote::QuoteWgsl(&#ident) });
            }
        }
    }
    if !current_string.is_empty() {
        format_arguments.push(proc_macro2::Literal::string(&current_string).to_token_stream());
        current_string.clear();
    }

    let format_string = "{}".repeat(format_arguments.len());
    quote::quote! {
        format!(#format_string, #(#format_arguments),*)
            .parse::<#ty>()
            .unwrap()
    }
}

fn quote_impl_inline(kind: QuoteNodeKind, input: TokenStream) -> TokenStream {
    let shader_code = Lexer::new(input.clone())
        .map(|token| token.as_wgsl_code().to_owned())
        .collect::<String>();

    match kind {
        QuoteNodeKind::TranslationUnit => {
            match wgsl_parse::syntax::TranslationUnit::from_str(&shader_code) {
                Ok(_) => {
                    to_wesl_tokens(input, quote::quote! { wgsl_parse::syntax::TranslationUnit })
                }
                Err(e) => {
                    abort!(get_error_span(input, e.span), "{}", e)
                }
            }
        }
        QuoteNodeKind::Literal => {
            match wgsl_parse::syntax::LiteralExpression::from_str(&shader_code) {
                Ok(_) => to_wesl_tokens(
                    input,
                    quote::quote! { wgsl_parse::syntax::LiteralExpression },
                ),
                Err(e) => {
                    abort!(get_error_span(input, e.span), "{}", e)
                }
            }
        }
        QuoteNodeKind::Expression => match wgsl_parse::syntax::Expression::from_str(&shader_code) {
            Ok(_) => to_wesl_tokens(input, quote::quote! { wgsl_parse::syntax::Expression }),
            Err(e) => {
                abort!(get_error_span(input, e.span), "{}", e)
            }
        },
        QuoteNodeKind::Statement => match wgsl_parse::syntax::Statement::from_str(&shader_code) {
            Ok(_) => to_wesl_tokens(input, quote::quote! { wgsl_parse::syntax::Statement }),
            Err(e) => {
                abort!(get_error_span(input, e.span), "{}", e)
            }
        },
    }
}

fn quote_impl_str(kind: QuoteNodeKind, input: &str, lit: &Literal) -> TokenStream {
    fn error_span(lit: &Literal, e: &wgsl_parse::Error) -> proc_macro2::Span {
        lit.subspan(e.span.range())
            .unwrap_or_else(proc_macro2::Span::call_site)
    }
    match kind {
        QuoteNodeKind::TranslationUnit => {
            match wgsl_parse::syntax::TranslationUnit::from_str(input) {
                Ok(_) => {
                    quote::quote! { #input.parse::<wgsl_parse::syntax::TranslationUnit>().unwrap() }
                }
                Err(e) => abort!(error_span(lit, &e), "{}", e),
            }
        }
        QuoteNodeKind::Literal => match wgsl_parse::syntax::LiteralExpression::from_str(input) {
            Ok(_) => {
                quote::quote! { #input.parse::<wgsl_parse::syntax::LiteralExpression>().unwrap() }
            }
            Err(e) => abort!(error_span(lit, &e), "{}", e),
        },
        QuoteNodeKind::Expression => match wgsl_parse::syntax::Expression::from_str(input) {
            Ok(_) => {
                quote::quote! { #input.parse::<wgsl_parse::syntax::Expression>().unwrap() }
            }
            Err(e) => abort!(error_span(lit, &e), "{}", e),
        },
        QuoteNodeKind::Statement => match wgsl_parse::syntax::Statement::from_str(input) {
            Ok(_) => {
                quote::quote! { #input.parse::<wgsl_parse::syntax::Statement>().unwrap() }
            }
            Err(e) => abort!(error_span(lit, &e), "{}", e),
        },
    }
}

pub(crate) fn quote_impl(kind: QuoteNodeKind, input: TokenStream) -> TokenStream {
    let mut token_stream = FlattenRec::from(input.clone().into_iter()).peekable();

    match token_stream.peek() {
        Some(RustToken::Literal(lit)) => {
            // TODO: Remove syn dependency once we have https://github.com/rust-lang/rust/issues/136652
            match syn::Lit::new(lit.clone()) {
                syn::Lit::Str(str) => quote_impl_str(kind, &str.value(), lit),
                _ => quote_impl_inline(kind, input),
            }
        }
        _ => quote_impl_inline(kind, input),
    }
}
