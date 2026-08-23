use std::iter::Peekable;

use itertools::Itertools;
use proc_macro_error2::{abort, abort_call_site};
use proc_macro2::{Ident, Literal, Punct, Spacing, TokenStream};
use token_stream_flatten::{
    Delimiter, DelimiterKind, DelimiterPosition, FlattenRec, Token as RustToken,
};
use wgsl_parse::{TokRepr, lexer::Token};

type Span = std::ops::Range<usize>;
type NextToken = Option<(Token, Span)>;

struct Lexer {
    token_stream: Peekable<FlattenRec>,
    next_token: NextToken,
    recognizing_template: bool,
    opened_templates: u32,
    token_counter: usize,
    /// Tokens produced by an interpolation marker (e.g. `#decl@ident`).
    pending: std::collections::VecDeque<(Token, Span)>,
    extras: LexerState,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct LexerState {
    depth: i32,
    template_depths: Vec<i32>,
    lookahead: Option<Token>,
}

fn maybe_template_end(lex: &mut Lexer, current: Token, lookahead: Option<Token>) -> Token {
    if let Some(depth) = lex.extras.template_depths.last() {
        // if found a ">" on the same nesting level as the opening "<", it is a template end.
        if lex.extras.depth == *depth {
            lex.extras.template_depths.pop();
            // if lookahead is GreaterThan, we may have a second closing template.
            // note that >>= can never be (TemplateEnd, TemplateEnd, Equal).
            if let Some(depth) = lex.extras.template_depths.last() {
                if lex.extras.depth == *depth && lookahead == Some(Token::SymGreaterThan) {
                    lex.extras.template_depths.pop();
                    lex.extras.lookahead = Some(Token::TemplateArgsEnd);
                } else {
                    lex.extras.lookahead = lookahead;
                }
            } else {
                lex.extras.lookahead = lookahead;
            }
            return Token::TemplateArgsEnd;
        }
    }

    current
}

// operators && and || have lower precedence than < and >.
// therefore, this is not a template: a < b || c > d
fn maybe_fail_template(lex: &mut Lexer) -> bool {
    if let Some(depth) = lex.extras.template_depths.last()
        && lex.extras.depth == *depth
    {
        return false;
    }
    true
}

fn incr_depth(lex: &mut Lexer) {
    lex.extras.depth += 1;
}

fn decr_depth(lex: &mut Lexer) {
    lex.extras.depth -= 1;
}

fn delim2tok(lex: &mut Lexer, delim: &Delimiter) -> Token {
    match (delim.kind(), delim.position()) {
        (DelimiterKind::Brace, DelimiterPosition::Open) => Token::SymBraceLeft,
        (DelimiterKind::Brace, DelimiterPosition::Close) => Token::SymBraceRight,
        (DelimiterKind::Bracket, DelimiterPosition::Open) => {
            incr_depth(lex);
            Token::SymBracketLeft
        }
        (DelimiterKind::Bracket, DelimiterPosition::Close) => {
            decr_depth(lex);
            Token::SymBracketRight
        }
        (DelimiterKind::Parenthesis, DelimiterPosition::Open) => {
            incr_depth(lex);
            Token::SymParenLeft
        }
        (DelimiterKind::Parenthesis, DelimiterPosition::Close) => {
            decr_depth(lex);
            Token::SymParenRight
        }
    }
}

fn ident2tok(ident: Ident) -> Token {
    let repr = ident.to_string();
    match repr.as_str() {
        "alias" => Token::KwAlias,
        "break" => Token::KwBreak,
        "case" => Token::KwCase,
        "const" => Token::KwConst,
        "const_assert" => Token::KwConstAssert,
        "continue" => Token::KwContinue,
        "continuing" => Token::KwContinuing,
        "default" => Token::KwDefault,
        "diagnostic" => Token::KwDiagnostic,
        "discard" => Token::KwDiscard,
        "else" => Token::KwElse,
        "enable" => Token::KwEnable,
        "false" => Token::KwFalse,
        "fn" => Token::KwFn,
        "for" => Token::KwFor,
        "if" => Token::KwIf,
        "let" => Token::KwLet,
        "loop" => Token::KwLoop,
        "override" => Token::KwOverride,
        "requires" => Token::KwRequires,
        "return" => Token::KwReturn,
        "struct" => Token::KwStruct,
        "switch" => Token::KwSwitch,
        "true" => Token::KwTrue,
        "var" => Token::KwVar,
        "while" => Token::KwWhile,
        // #[cfg(feature = "imports")]
        "self" => Token::KwSelf,
        // #[cfg(feature = "imports")]
        "super" => Token::KwSuper,
        // #[cfg(feature = "imports")]
        "package" => Token::KwPackage,
        // #[cfg(feature = "imports")]
        "as" => Token::KwAs,
        // #[cfg(feature = "imports")]
        "import" => Token::KwImport,
        _ => Token::Ident(repr),
    }
}

fn lit2tok(lit: Literal) -> Token {
    match syn::Lit::new(lit) {
        syn::Lit::Int(lit) => match lit.suffix() {
            "" => Token::AbstractInt(
                lit.base10_parse::<i64>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            "i" => Token::I32(
                lit.base10_parse::<i32>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            "u" => Token::U32(
                lit.base10_parse::<u32>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            "f" => Token::F32(
                lit.base10_parse::<f32>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            "h" => Token::F16(
                // TODO validate that if fits in f16
                lit.base10_parse::<f32>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            _ => abort!(lit, "invalid literal suffix"),
        },
        syn::Lit::Float(lit) => match lit.suffix() {
            "" => Token::AbstractFloat(
                lit.base10_parse::<f64>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            "f" => Token::F32(
                lit.base10_parse::<f32>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            "h" => Token::F16(
                // TODO validate that if fits in f16
                lit.base10_parse::<f32>()
                    .unwrap_or_else(|e| abort!(lit, "invalid literal: {}", e)),
            ),
            _ => abort!(lit, "invalid literal suffix"),
        },
        syn::Lit::Bool(lit) => match lit.value() {
            true => Token::KwTrue,
            false => Token::KwFalse,
        },
        lit => abort!(lit, "invalid WESL token"),
    }
}

fn punct2tok(lex: &mut Lexer, punct: Punct, repr: &str) -> Token {
    match repr {
        "&" => Token::SymAnd,
        "&&" => {
            if maybe_fail_template(lex) {
                Token::SymAnd
            } else {
                abort!(punct, "invalid WESL punctuation `{}`", repr)
            }
        }
        "->" => Token::SymArrow,
        "@" => Token::SymAttr,
        "/" => Token::SymForwardSlash,
        "!" => Token::SymBang,
        "{" => Token::SymBraceLeft,
        "}" => Token::SymBraceRight,
        ":" => Token::SymColon,
        "," => Token::SymComma,
        "=" => Token::SymEqual,
        "==" => Token::SymEqualEqual,
        "!=" => Token::SymNotEqual,
        ">" => maybe_template_end(lex, Token::SymGreaterThan, None),
        ">=" => maybe_template_end(lex, Token::SymGreaterThanEqual, Some(Token::SymEqual)),
        ">>" => maybe_template_end(lex, Token::SymShiftRight, Some(Token::SymGreaterThan)),
        "<" => Token::SymLessThan,
        "<=" => Token::SymLessThanEqual,
        "<<" => Token::SymShiftLeft,
        "%" => Token::SymModulo,
        "-" => Token::SymMinus,
        "--" => Token::SymMinusMinus,
        "." => Token::SymPeriod,
        "+" => Token::SymPlus,
        "++" => Token::SymPlusPlus,
        "|" => Token::SymOr,
        "||" => {
            if maybe_fail_template(lex) {
                Token::SymOrOr
            } else {
                abort!(punct, "invalid WESL punctuation `{}`", repr)
            }
        }
        ";" => Token::SymSemicolon,
        "*" => Token::SymStar,
        "~" => Token::SymTilde,
        "_" => Token::SymUnderscore,
        "^" => Token::SymXor,
        "+=" => Token::SymPlusEqual,
        "-=" => Token::SymMinusEqual,
        "*=" => Token::SymTimesEqual,
        "/=" => Token::SymDivisionEqual,
        "%=" => Token::SymModuloEqual,
        "&=" => Token::SymAndEqual,
        "|=" => Token::SymOrEqual,
        "^=" => Token::SymXorEqual,
        ">>=" => maybe_template_end(
            lex,
            Token::SymShiftRightAssign,
            Some(Token::SymGreaterThanEqual),
        ),
        "<<=" => Token::SymShiftLeftAssign,
        // #[cfg(feature = "imports")]
        "::" => Token::SymColonColon,
        _ => abort!(punct, "invalid WESL punctuation `{}`", repr),
    }
}

pub fn recognize_template_list(token_stream: Peekable<FlattenRec>, offset: usize) -> bool {
    let start_span = offset..offset + 1;
    let mut lexer = Lexer::new(token_stream, Some((Token::TemplateArgsStart, start_span)));
    lexer.recognizing_template = true;
    lexer.opened_templates = 1;
    lexer.extras.template_depths.push(0);
    wgsl_parse::parser::recognize_template_list(lexer).is_ok()
}

impl Lexer {
    fn new(token_stream: Peekable<FlattenRec>, next_token: NextToken) -> Self {
        let mut lex = Self {
            token_stream,
            next_token,
            recognizing_template: false,
            opened_templates: 0,
            token_counter: 0,
            pending: Default::default(),
            extras: Default::default(),
        };
        if lex.next_token.is_none() {
            lex.next_token = lex.wesl_next_token();
        }
        lex
    }

    /// Pull the next WESL token, draining any pending interpolation tokens first.
    fn wesl_next_token(&mut self) -> NextToken {
        if let Some(tok) = self.pending.pop_front() {
            return Some(tok);
        }
        self.rust_next_token()
            .and_then(|(tok, off)| self.tok2wesl(tok, off))
    }

    fn rust_next_token(&mut self) -> Option<(RustToken, usize)> {
        let tok = self.token_stream.next()?;
        let offset = self.token_counter;
        self.token_counter += 1;
        Some((tok, offset))
    }

    fn take_two_tokens(&mut self) -> (NextToken, NextToken) {
        let tok1 = self.next_token.take();

        let lookahead = self.extras.lookahead.take();
        let tok2 = match lookahead {
            Some(tok) => {
                let (_, span) = tok1.as_ref().unwrap(); // safety: lookahead implies lexer looked at a `<` token
                Some((tok, span.clone()))
            }
            None => self.wesl_next_token(),
        };

        (tok1, tok2)
    }

    fn tok2wesl(&mut self, tok: RustToken, offset: usize) -> NextToken {
        let mut span = offset..offset + 1;
        match tok {
            RustToken::Delimiter(delim) => Some((delim2tok(self, &delim), span)),
            RustToken::Ident(id) => {
                let tok = ident2tok(id);
                Some((tok, span))
            }
            RustToken::Literal(lit) => {
                let tok = lit2tok(lit);
                Some((tok, span))
            }
            RustToken::Punct(punct) => {
                let mut repr = punct.to_string();
                if repr == "#" {
                    match self.rust_next_token()? {
                        (RustToken::Ident(id), off) => {
                            span.end = off + 1;
                            self.marker2wesl(&id.to_string(), span)
                        }
                        (tok, _) => {
                            abort!(tok.span(), "expected an interpolation marker after `#`",)
                        }
                    }
                } else {
                    let mut join_punct = punct.spacing() == Spacing::Joint;
                    while join_punct {
                        match self.token_stream.peek().unwrap() {
                            RustToken::Punct(punct) => {
                                // TODO: this is not ideal, we should check if it forms a valid lit.
                                let chr = punct.as_char();
                                if ".;,#".chars().contains(&chr) {
                                    join_punct = false;
                                } else {
                                    repr.push(chr);
                                    join_punct = punct.spacing() == Spacing::Joint;
                                    let (_, offset) = self.rust_next_token().unwrap();
                                    span.end = offset + 1;
                                }
                            }
                            tok => abort!(tok.span(), "unreachable"),
                        };
                    }
                    Some((punct2tok(self, punct, &repr), span))
                }
            }
        }
    }

    /// Expand a variable interpolation composed of a marker and a variable identifier (e.g. `#expr@ident`).
    fn marker2wesl(&mut self, marker: &str, mut span: Span) -> NextToken {
        let is_marker = matches!(marker, "expr" | "decl" | "stmt" | "mem" | "attr")
            && matches!(self.token_stream.peek(), Some(RustToken::Punct(p)) if p.as_char() == '@');

        if !is_marker {
            // no marker (#ident) is equivalent to #expr@ident.
            return Some((Token::Ident(format!("#{marker}")), span));
        }

        self.rust_next_token(); // consume the `@`.

        // consume the variable ident.
        let (ident, off) = match self.rust_next_token() {
            Some((RustToken::Ident(id), off)) => (id.to_string(), off),
            Some((tok, _)) => abort!(tok.span(), "expected an identifier after `#{}@`", marker),
            None => abort_call_site!("expected an identifier after `#{}@`", marker),
        };
        span.end = off + 1;

        let inject = Token::Ident(format!("#{ident}"));

        // build the token sequence for the marker. the first token is returned, the rest
        // are queued in `self.pending`.
        let (tok, pending): (Token, Vec<Token>) = match marker {
            "expr" => {
                // `#ident`
                (inject, vec![])
            }
            "decl" | "stmt" => {
                // `const #ident = __dummy_interpolation ;`
                (
                    Token::KwConst,
                    vec![
                        inject,
                        Token::SymEqual,
                        Token::Ident("__dummy_interpolation".to_string()),
                        Token::SymSemicolon,
                    ],
                )
            }
            "mem" => {
                // `#ident : __dummy_interpolation`
                (
                    inject,
                    vec![
                        Token::SymColon,
                        Token::Ident("__dummy_interpolation".to_string()),
                    ],
                )
            }
            "attr" => {
                // `@ #ident`
                (Token::SymAttr, vec![inject])
            }
            _ => unreachable!("not a valid marker name"),
        };

        self.pending
            .extend(pending.into_iter().map(|tok| (tok, span.clone())));

        Some((tok, span))
    }

    fn next_tok(&mut self) -> Option<(Token, Span)> {
        let (cur, mut next) = self.take_two_tokens();

        let (cur_tok, cur_span) = cur?;

        if let Some((next_tok, offset)) = &mut next
            && (matches!(cur_tok, Token::Ident(_)) || cur_tok.is_keyword())
            && *next_tok == Token::SymLessThan
        {
            let input = self.token_stream.clone();
            if recognize_template_list(input, offset.start) {
                *next_tok = Token::TemplateArgsStart;
                let cur_depth = self.extras.depth;
                self.extras.template_depths.push(cur_depth);
                self.opened_templates += 1;
            }
        }

        // if we finished recognition of a template
        if self.recognizing_template && cur_tok == Token::TemplateArgsEnd {
            self.opened_templates -= 1;
            if self.opened_templates == 0 {
                next = None; // push eof after end of template
            }
        }

        self.next_token = next;
        Some((cur_tok, cur_span))
    }
}

type Spanned<Tok, Loc, ParseError> = Result<(Loc, Tok, Loc), (Loc, ParseError, Loc)>;

impl Iterator for Lexer {
    type Item = Spanned<Token, usize, wgsl_parse::error::ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (tok, span) = self.next_tok()?;
        Some(Ok((span.start, tok, span.end)))
    }
}

impl wgsl_parse::lexer::TokenIterator for Lexer {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuoteNodeKind {
    TranslationUnit,
    ImportStatement,
    GlobalDeclaration,
    Literal,
    GlobalDirective,
    Expression,
    Statement,
}

fn quote_impl_inline(kind: QuoteNodeKind, input: TokenStream) -> TokenStream {
    use wgsl_parse::parser::{ParseEntryPoint, parse_tokens};
    let token_stream = FlattenRec::from(input.clone().into_iter()).peekable();
    let lexer = Lexer::new(token_stream, None);

    macro_rules! parser_impl {
        ($token:ident, $entrypoint:ident) => {{
            match parse_tokens(lexer, Token::$token) {
                Ok(ParseEntryPoint::$entrypoint(res)) => res.tok_repr(),
                Ok(_) => unreachable!("parser parsed the wrong entrypoint"),
                Err(e) => {
                    let err = wgsl_parse::Error::from(e);
                    let span = err.span;
                    let mut token_stream = FlattenRec::from(input.into_iter());
                    let start = token_stream
                        .nth(span.start)
                        .map(|tok| tok.span())
                        .unwrap_or(proc_macro2::Span::call_site());
                    // let end = token_stream
                    //     .nth(span.end - span.start - 1)
                    //     .map(|tok| tok.span())
                    //     .unwrap_or(proc_macro2::Span::call_site());
                    abort!(start, "{}", err)
                }
            }
        }};
    }

    match kind {
        QuoteNodeKind::TranslationUnit => parser_impl!(EntryPointTranslationUnit, TranslationUnit),
        QuoteNodeKind::ImportStatement => parser_impl!(EntryPointImportStatement, ImportStatement),
        QuoteNodeKind::GlobalDeclaration => parser_impl!(EntryPointGlobalDecl, GlobalDecl),
        QuoteNodeKind::Literal => parser_impl!(EntryPointLiteral, Literal),
        QuoteNodeKind::GlobalDirective => parser_impl!(EntryPointGlobalDirective, GlobalDirective),
        QuoteNodeKind::Expression => parser_impl!(EntryPointExpression, Expression),
        QuoteNodeKind::Statement => parser_impl!(EntryPointStatement, Statement),
    }
}

fn quote_impl_str(kind: QuoteNodeKind, str: &str) -> TokenStream {
    use wgsl_parse::parser::{ParseEntryPoint, parse_tokens};
    let lexer = wgsl_parse::lexer::Lexer::new(str);

    macro_rules! parser_impl {
        ($token:ident, $entrypoint:ident) => {{
            match parse_tokens(lexer, Token::$token) {
                Ok(ParseEntryPoint::$entrypoint(res)) => res.tok_repr(),
                Ok(_) => unreachable!("parser parsed the wrong entrypoint"),
                Err(e) => {
                    let err = wgsl_parse::Error::from(e);
                    abort_call_site!("{}", err)
                }
            }
        }};
    }

    match kind {
        QuoteNodeKind::TranslationUnit => parser_impl!(EntryPointTranslationUnit, TranslationUnit),
        QuoteNodeKind::ImportStatement => parser_impl!(EntryPointImportStatement, ImportStatement),
        QuoteNodeKind::GlobalDeclaration => parser_impl!(EntryPointGlobalDecl, GlobalDecl),
        QuoteNodeKind::Literal => parser_impl!(EntryPointLiteral, Literal),
        QuoteNodeKind::GlobalDirective => parser_impl!(EntryPointGlobalDirective, GlobalDirective),
        QuoteNodeKind::Expression => parser_impl!(EntryPointExpression, Expression),
        QuoteNodeKind::Statement => parser_impl!(EntryPointStatement, Statement),
    }
}

pub(crate) fn quote_impl(kind: QuoteNodeKind, input: TokenStream) -> TokenStream {
    let mut token_stream = FlattenRec::from(input.clone().into_iter()).peekable();
    match token_stream.peek() {
        Some(RustToken::Literal(lit)) => match syn::Lit::new(lit.clone()) {
            syn::Lit::Str(str) => quote_impl_str(kind, &str.value()),
            _ => quote_impl_inline(kind, input),
        },
        _ => quote_impl_inline(kind, input),
    }
}
