//! Prefer using [`crate::parse_str`]. You shouldn't need to manipulate the lexer.

use crate::error::ParseError;
use itertools::Itertools;
use logos::{Logos, SpannedIter};
use std::{fmt::Display, num::NonZeroU8, sync::LazyLock};
use wgsl_types::idents::RESERVED_WORDS;

type Span = std::ops::Range<usize>;

fn maybe_template_end(
    lex: &mut logos::Lexer<Token>,
    current: Token,
    lookahead: Option<Token>,
) -> Token {
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
fn maybe_fail_template(lex: &mut logos::Lexer<Token>) -> bool {
    if let Some(depth) = lex.extras.template_depths.last() {
        if lex.extras.depth == *depth {
            return false;
        }
    }
    true
}

fn incr_depth(lex: &mut logos::Lexer<Token>) {
    lex.extras.depth += 1;
}

fn decr_depth(lex: &mut logos::Lexer<Token>) {
    lex.extras.depth -= 1;
}

// TODO: get rid of crate `lexical`

// don't have to be super strict, the lexer regex already did the heavy lifting
const DEC_FORMAT: u128 = lexical::NumberFormatBuilder::new().build();

// don't have to be super strict, the lexer regex already did the heavy lifting
const HEX_FORMAT: u128 = lexical::NumberFormatBuilder::new()
    .mantissa_radix(16)
    .base_prefix(NonZeroU8::new(b'x'))
    .exponent_base(NonZeroU8::new(16))
    .exponent_radix(NonZeroU8::new(10))
    .build();

static FLOAT_HEX_OPTIONS: LazyLock<lexical::parse_float_options::Options> = LazyLock::new(|| {
    lexical::parse_float_options::OptionsBuilder::new()
        .exponent(b'p')
        .decimal_point(b'.')
        .build()
        .unwrap()
});

fn parse_dec_abstract_int(lex: &mut logos::Lexer<Token>) -> Option<i64> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    lexical::parse_with_options::<i64, _, DEC_FORMAT>(str, options).ok()
}

fn parse_hex_abstract_int(lex: &mut logos::Lexer<Token>) -> Option<i64> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    lexical::parse_with_options::<i64, _, HEX_FORMAT>(str, options).ok()
}

fn parse_dec_i32(lex: &mut logos::Lexer<Token>) -> Option<i32> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<i32, _, DEC_FORMAT>(str, options).ok()
}

fn parse_hex_i32(lex: &mut logos::Lexer<Token>) -> Option<i32> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<i32, _, HEX_FORMAT>(str, options).ok()
}

fn parse_dec_u32(lex: &mut logos::Lexer<Token>) -> Option<u32> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<u32, _, DEC_FORMAT>(str, options).ok()
}

fn parse_hex_u32(lex: &mut logos::Lexer<Token>) -> Option<u32> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<u32, _, HEX_FORMAT>(str, options).ok()
}

fn parse_dec_abs_float(lex: &mut logos::Lexer<Token>) -> Option<f64> {
    let options = &lexical::parse_float_options::STANDARD;
    let str = lex.slice();
    lexical::parse_with_options::<f64, _, DEC_FORMAT>(str, options).ok()
}

fn parse_hex_abs_float(lex: &mut logos::Lexer<Token>) -> Option<f64> {
    let str = lex.slice();
    lexical::parse_with_options::<f64, _, HEX_FORMAT>(str, &FLOAT_HEX_OPTIONS).ok()
}

fn parse_dec_f32(lex: &mut logos::Lexer<Token>) -> Option<f32> {
    let options = &lexical::parse_float_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<f32, _, DEC_FORMAT>(str, options).ok()
}

fn parse_hex_f32(lex: &mut logos::Lexer<Token>) -> Option<f32> {
    let str = lex.slice();
    // TODO
    let options = &lexical::parse_float_options::STANDARD;
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<f32, _, HEX_FORMAT>(str, options).ok()
}

fn parse_dec_f16(lex: &mut logos::Lexer<Token>) -> Option<f32> {
    let options = &lexical::parse_float_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<f32, _, DEC_FORMAT>(str, options).ok()
}

fn parse_hex_f16(lex: &mut logos::Lexer<Token>) -> Option<f32> {
    let str = lex.slice();
    let str = &str[..str.len() - 1];
    lexical::parse_with_options::<f32, _, HEX_FORMAT>(str, &FLOAT_HEX_OPTIONS).ok()
}

fn parse_dec_i64(lex: &mut logos::Lexer<Token>) -> Option<i64> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 2];
    lexical::parse_with_options::<i64, _, DEC_FORMAT>(str, options).ok()
}

// extension: naga-ext
fn parse_hex_i64(lex: &mut logos::Lexer<Token>) -> Option<i64> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 2];
    lexical::parse_with_options::<i64, _, HEX_FORMAT>(str, options).ok()
}

// extension: naga-ext
fn parse_dec_u64(lex: &mut logos::Lexer<Token>) -> Option<u64> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 2];
    lexical::parse_with_options::<u64, _, DEC_FORMAT>(str, options).ok()
}

// extension: naga-ext
fn parse_hex_u64(lex: &mut logos::Lexer<Token>) -> Option<u64> {
    let options = &lexical::parse_integer_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 2];
    lexical::parse_with_options::<u64, _, HEX_FORMAT>(str, options).ok()
}

// extension: naga-ext
fn parse_dec_f64(lex: &mut logos::Lexer<Token>) -> Option<f64> {
    let options = &lexical::parse_float_options::STANDARD;
    let str = lex.slice();
    let str = &str[..str.len() - 2];
    lexical::parse_with_options::<f64, _, DEC_FORMAT>(str, options).ok()
}

// extension: naga-ext
fn parse_hex_f64(lex: &mut logos::Lexer<Token>) -> Option<f64> {
    let str = lex.slice();
    // TODO
    let options = &lexical::parse_float_options::STANDARD;
    let str = &str[..str.len() - 2];
    lexical::parse_with_options::<f64, _, HEX_FORMAT>(str, options).ok()
}

fn parse_line_comment(lex: &mut logos::Lexer<Token>) {
    let rem = lex.remainder();
    // see blankspace and line breaks: https://www.w3.org/TR/WGSL/#blankspace-and-line-breaks
    let line_end = rem
        .char_indices()
        .find(|(_, c)| "\n\u{000B}\u{000C}\r\u{0085}\u{2028}\u{2029}".contains(*c))
        .map(|(i, _)| i)
        .unwrap_or(rem.len());
    lex.bump(line_end);
}

fn parse_block_comment(lex: &mut logos::Lexer<Token>) {
    let mut depth = 1;
    while depth > 0 {
        let rem = lex.remainder();
        if rem.is_empty() {
            break;
        } else if rem.starts_with("/*") {
            lex.bump(2);
            depth += 1;
        } else if rem.starts_with("*/") {
            lex.bump(2);
            depth -= 1;
        } else {
            let mut next_char = 1;
            while !rem.is_char_boundary(next_char) {
                next_char += 1;
            }
            lex.bump(next_char);
        }
    }
}

fn parse_ident(lex: &mut logos::Lexer<Token>) -> Token {
    let ident = lex.slice().to_string();
    if RESERVED_WORDS.iter().contains(&ident.as_str()) {
        Token::ReservedWord(ident)
    } else {
        Token::Ident(ident)
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct LexerState {
    depth: i32,
    template_depths: Vec<i32>,
    lookahead: Option<Token>,
}

// following the spec at this date: https://www.w3.org/TR/2024/WD-WGSL-20240731/
#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(
    // see blankspace and line breaks: https://www.w3.org/TR/WGSL/#blankspace-and-line-breaks
    skip r"[\s\u0085\u200e\u200f\u2028\u2029]+", // blankspace
    extras = LexerState,
    error = ParseError)]
pub enum Token {
    #[token("//", parse_line_comment)]
    LineComment,
    #[token("/*", parse_block_comment, priority = 2)]
    BlockComment,
    // the parse_ident function can return either Token::Ident or Token::ReservedWord.
    // Token::Ignored variant is never produced.
    // It serves as a placeholder for running parse_ident.
    #[regex(
        r#"([_\p{XID_Start}][\p{XID_Continue}]+)|([\p{XID_Start}])"#,
        parse_ident,
        priority = 1
    )]
    Ignored,
    // syntactic tokens
    // https://www.w3.org/TR/WGSL/#syntactic-tokens
    #[token("&")]
    SymAnd,
    #[token("&&", maybe_fail_template)]
    SymAndAnd,
    #[token("->")]
    SymArrow,
    #[token("@")]
    SymAttr,
    #[token("/")]
    SymForwardSlash,
    #[token("!")]
    SymBang,
    #[token("[", incr_depth)]
    SymBracketLeft,
    #[token("]", decr_depth)]
    SymBracketRight,
    #[token("{")]
    SymBraceLeft,
    #[token("}")]
    SymBraceRight,
    #[token(":")]
    SymColon,
    #[token(",")]
    SymComma,
    #[token("=")]
    SymEqual,
    #[token("==")]
    SymEqualEqual,
    #[token("!=")]
    SymNotEqual,
    #[token(">", |lex| maybe_template_end(lex, Token::SymGreaterThan, None))]
    SymGreaterThan,
    #[token(">=", |lex| maybe_template_end(lex, Token::SymGreaterThanEqual, Some(Token::SymEqual)))]
    SymGreaterThanEqual,
    #[token(">>", |lex| maybe_template_end(lex, Token::SymShiftRight, Some(Token::SymGreaterThan)))]
    SymShiftRight,
    #[token("<")]
    SymLessThan,
    #[token("<=")]
    SymLessThanEqual,
    #[token("<<")]
    SymShiftLeft,
    #[token("%")]
    SymModulo,
    #[token("-")]
    SymMinus,
    #[token("--")]
    SymMinusMinus,
    #[token(".")]
    SymPeriod,
    #[token("+")]
    SymPlus,
    #[token("++")]
    SymPlusPlus,
    #[token("|")]
    SymOr,
    #[token("||", maybe_fail_template)]
    SymOrOr,
    #[token("(", incr_depth)]
    SymParenLeft,
    #[token(")", decr_depth)]
    SymParenRight,
    #[token(";")]
    SymSemicolon,
    #[token("*")]
    SymStar,
    #[token("~")]
    SymTilde,
    #[token("_")]
    SymUnderscore,
    #[token("^")]
    SymXor,
    #[token("+=")]
    SymPlusEqual,
    #[token("-=")]
    SymMinusEqual,
    #[token("*=")]
    SymTimesEqual,
    #[token("/=")]
    SymDivisionEqual,
    #[token("%=")]
    SymModuloEqual,
    #[token("&=")]
    SymAndEqual,
    #[token("|=")]
    SymOrEqual,
    #[token("^=")]
    SymXorEqual,
    #[token(">>=", |lex| maybe_template_end(lex, Token::SymShiftRightAssign, Some(Token::SymGreaterThanEqual)))]
    SymShiftRightAssign,
    #[token("<<=")]
    SymShiftLeftAssign,

    // keywords
    // https://www.w3.org/TR/WGSL/#keyword-summary
    #[token("alias")]
    KwAlias,
    #[token("break")]
    KwBreak,
    #[token("case")]
    KwCase,
    #[token("const", priority = 2)]
    KwConst,
    #[token("const_assert")]
    KwConstAssert,
    #[token("continue")]
    KwContinue,
    #[token("continuing")]
    KwContinuing,
    #[token("default")]
    KwDefault,
    #[token("diagnostic")]
    KwDiagnostic,
    #[token("discard")]
    KwDiscard,
    #[token("else")]
    KwElse,
    #[token("enable")]
    KwEnable,
    #[token("false")]
    KwFalse,
    #[token("fn")]
    KwFn,
    #[token("for")]
    KwFor,
    #[token("if")]
    KwIf,
    #[token("let")]
    KwLet,
    #[token("loop")]
    KwLoop,
    #[token("override")]
    KwOverride,
    #[token("requires")]
    KwRequires,
    #[token("return")]
    KwReturn,
    #[token("struct")]
    KwStruct,
    #[token("switch")]
    KwSwitch,
    #[token("true")]
    KwTrue,
    #[token("var")]
    KwVar,
    #[token("while")]
    KwWhile,

    // Idents and ReservedWord tokens are parsed on the Ignored variant, because of a current
    // limitation of logos. See logos#295.
    Ident(String),
    // variant produced by parse_ident for reserved words.
    // Reserved words can be used in context-dependent words, e.g. attribute names and module names.
    ReservedWord(String),

    #[regex(r#"0|[1-9]\d*"#, parse_dec_abstract_int)]
    #[regex(r#"0[xX][\da-fA-F]+"#, parse_hex_abstract_int)]
    AbstractInt(i64),
    #[regex(r#"(\d+\.\d*|\.\d+)([eE][+-]?\d+)?"#, parse_dec_abs_float)]
    #[regex(r#"\d+[eE][+-]?\d+"#, parse_dec_abs_float)]
    #[regex(r#"0[xX][\da-fA-F]+\.[\da-fA-F]*([pP][+-]?\d+)?"#, parse_hex_abs_float)]
    #[regex(r#"0[xX]\.[\da-fA-F]+([pP][+-]?\d+)?"#, parse_hex_abs_float)]
    #[regex(r#"0[xX][\da-fA-F]+[pP][+-]?\d+"#, parse_hex_abs_float)]
    // hex
    AbstractFloat(f64),
    #[regex(r#"(0|[1-9]\d*)i"#, parse_dec_i32)]
    #[regex(r#"0[xX][\da-fA-F]+i"#, parse_hex_i32)]
    I32(i32),
    #[regex(r#"(0|[1-9]\d*)u"#, parse_dec_u32)]
    #[regex(r#"0[xX][\da-fA-F]+u"#, parse_hex_u32)]
    U32(u32),
    #[regex(r#"(\d+\.\d*|\.\d+)([eE][+-]?\d+)?f"#, parse_dec_f32)]
    #[regex(r#"\d+([eE][+-]?\d+)?f"#, parse_dec_f32)]
    #[regex(r#"0[xX][\da-fA-F]+\.[\da-fA-F]*[pP][+-]?\d+f"#, parse_hex_f32)]
    #[regex(r#"0[xX]\.[\da-fA-F]+[pP][+-]?\d+f"#, parse_hex_f32)]
    #[regex(r#"0[xX][\da-fA-F]+[pP][+-]?\d+f"#, parse_hex_f32)]
    F32(f32),
    #[regex(r#"(\d+\.\d*|\.\d+)([eE][+-]?\d+)?h"#, parse_dec_f16)]
    #[regex(r#"\d+([eE][+-]?\d+)?h"#, parse_dec_f16)]
    #[regex(r#"0[xX][\da-fA-F]+\.[\da-fA-F]*[pP][+-]?\d+h"#, parse_hex_f16)]
    #[regex(r#"0[xX]\.[\da-fA-F]+[pP][+-]?\d+h"#, parse_hex_f16)]
    #[regex(r#"0[xX][\da-fA-F]+[pP][+-]?\d+h"#, parse_hex_f16)]
    F16(f32),

    TemplateArgsStart,
    TemplateArgsEnd,

    // extension: naga-ext
    #[regex(r#"(0|[1-9]\d*)li"#, parse_dec_i64)]
    #[regex(r#"0[xX][\da-fA-F]+li"#, parse_hex_i64)]
    I64(i64),
    // extension: naga-ext
    #[regex(r#"(0|[1-9]\d*)lu"#, parse_dec_u64)]
    #[regex(r#"0[xX][\da-fA-F]+lu"#, parse_hex_u64)]
    // hex
    U64(u64),
    // extension: naga-ext
    #[regex(r#"(\d+\.\d*|\.\d+)([eE][+-]?\d+)?lf"#, parse_dec_f64)]
    #[regex(r#"\d+([eE][+-]?\d+)?lf"#, parse_dec_f64)]
    #[regex(r#"0[xX][\da-fA-F]+\.[\da-fA-F]*[pP][+-]?\d+lf"#, parse_hex_f64)]
    #[regex(r#"0[xX]\.[\da-fA-F]+[pP][+-]?\d+lf"#, parse_hex_f64)]
    #[regex(r#"0[xX][\da-fA-F]+[pP][+-]?\d+lf"#, parse_hex_f64)]
    F64(f64),

    // extension: wesl-imports
    // https://github.com/wgsl-tooling-wg/wesl-spec/blob/imports-update/Imports.md
    // date: 2025-01-18, hash: 2db8e7f681087db6bdcd4a254963deb5c0159775
    #[token("::")]
    SymColonColon,
    #[token("self")]
    KwSelf,
    #[token("super")]
    KwSuper,
    #[token("package")]
    KwPackage,
    #[token("as")]
    KwAs,
    #[token("import")]
    KwImport,
}

impl Token {
    pub fn is_trivia(&self) -> bool {
        matches!(
            self,
            Token::LineComment | Token::BlockComment | Token::Ignored
        )
    }

    #[allow(unused)]
    pub fn is_symbol(&self) -> bool {
        matches!(
            self,
            Token::SymAnd
                | Token::SymAndAnd
                | Token::SymArrow
                | Token::SymAttr
                | Token::SymForwardSlash
                | Token::SymBang
                | Token::SymBracketLeft
                | Token::SymBracketRight
                | Token::SymBraceLeft
                | Token::SymBraceRight
                | Token::SymColon
                | Token::SymComma
                | Token::SymEqual
                | Token::SymEqualEqual
                | Token::SymNotEqual
                | Token::SymGreaterThan
                | Token::SymGreaterThanEqual
                | Token::SymShiftRight
                | Token::SymLessThan
                | Token::SymLessThanEqual
                | Token::SymShiftLeft
                | Token::SymModulo
                | Token::SymMinus
                | Token::SymMinusMinus
                | Token::SymPeriod
                | Token::SymPlus
                | Token::SymPlusPlus
                | Token::SymOr
                | Token::SymOrOr
                | Token::SymParenLeft
                | Token::SymParenRight
                | Token::SymSemicolon
                | Token::SymStar
                | Token::SymTilde
                | Token::SymUnderscore
                | Token::SymXor
                | Token::SymPlusEqual
                | Token::SymMinusEqual
                | Token::SymTimesEqual
                | Token::SymDivisionEqual
                | Token::SymModuloEqual
                | Token::SymAndEqual
                | Token::SymOrEqual
                | Token::SymXorEqual
                | Token::SymShiftRightAssign
                | Token::SymShiftLeftAssign
        )
    }

    #[allow(unused)]
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Token::KwAlias
                | Token::KwBreak
                | Token::KwCase
                | Token::KwConst
                | Token::KwConstAssert
                | Token::KwContinue
                | Token::KwContinuing
                | Token::KwDefault
                | Token::KwDiagnostic
                | Token::KwDiscard
                | Token::KwElse
                | Token::KwEnable
                | Token::KwFalse
                | Token::KwFn
                | Token::KwFor
                | Token::KwIf
                | Token::KwLet
                | Token::KwLoop
                | Token::KwOverride
                | Token::KwRequires
                | Token::KwReturn
                | Token::KwStruct
                | Token::KwSwitch
                | Token::KwTrue
                | Token::KwVar
                | Token::KwWhile
        )
    }

    #[allow(unused)]
    pub fn is_numeric_literal(&self) -> bool {
        matches!(
            self,
            Token::AbstractInt(_)
                | Token::AbstractFloat(_)
                | Token::I32(_)
                | Token::U32(_)
                | Token::F32(_)
                | Token::F16(_)
        )
    }
}

impl Display for Token {
    /// This display implementation is used for error messages.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::LineComment => f.write_str("// line comment"),
            Token::BlockComment => f.write_str("/* block comment */"),
            Token::Ignored => unreachable!(),
            Token::SymAnd => f.write_str("&"),
            Token::SymAndAnd => f.write_str("&&"),
            Token::SymArrow => f.write_str("->"),
            Token::SymAttr => f.write_str("@"),
            Token::SymForwardSlash => f.write_str("/"),
            Token::SymBang => f.write_str("!"),
            Token::SymBracketLeft => f.write_str("["),
            Token::SymBracketRight => f.write_str("]"),
            Token::SymBraceLeft => f.write_str("{"),
            Token::SymBraceRight => f.write_str("}"),
            Token::SymColon => f.write_str(":"),
            Token::SymComma => f.write_str(","),
            Token::SymEqual => f.write_str("="),
            Token::SymEqualEqual => f.write_str("=="),
            Token::SymNotEqual => f.write_str("!="),
            Token::SymGreaterThan => f.write_str(">"),
            Token::SymGreaterThanEqual => f.write_str(">="),
            Token::SymShiftRight => f.write_str(">>"),
            Token::SymLessThan => f.write_str("<"),
            Token::SymLessThanEqual => f.write_str("<="),
            Token::SymShiftLeft => f.write_str("<<"),
            Token::SymModulo => f.write_str("%"),
            Token::SymMinus => f.write_str("-"),
            Token::SymMinusMinus => f.write_str("--"),
            Token::SymPeriod => f.write_str("."),
            Token::SymPlus => f.write_str("+"),
            Token::SymPlusPlus => f.write_str("++"),
            Token::SymOr => f.write_str("|"),
            Token::SymOrOr => f.write_str("||"),
            Token::SymParenLeft => f.write_str("("),
            Token::SymParenRight => f.write_str(")"),
            Token::SymSemicolon => f.write_str(";"),
            Token::SymStar => f.write_str("*"),
            Token::SymTilde => f.write_str("~"),
            Token::SymUnderscore => f.write_str("_"),
            Token::SymXor => f.write_str("^"),
            Token::SymPlusEqual => f.write_str("+="),
            Token::SymMinusEqual => f.write_str("-="),
            Token::SymTimesEqual => f.write_str("*="),
            Token::SymDivisionEqual => f.write_str("/="),
            Token::SymModuloEqual => f.write_str("%="),
            Token::SymAndEqual => f.write_str("&="),
            Token::SymOrEqual => f.write_str("|="),
            Token::SymXorEqual => f.write_str("^="),
            Token::SymShiftRightAssign => f.write_str(">>="),
            Token::SymShiftLeftAssign => f.write_str("<<="),
            Token::KwAlias => f.write_str("alias"),
            Token::KwBreak => f.write_str("break"),
            Token::KwCase => f.write_str("case"),
            Token::KwConst => f.write_str("const"),
            Token::KwConstAssert => f.write_str("const_assert"),
            Token::KwContinue => f.write_str("continue"),
            Token::KwContinuing => f.write_str("continuing"),
            Token::KwDefault => f.write_str("default"),
            Token::KwDiagnostic => f.write_str("diagnostic"),
            Token::KwDiscard => f.write_str("discard"),
            Token::KwElse => f.write_str("else"),
            Token::KwEnable => f.write_str("enable"),
            Token::KwFalse => f.write_str("false"),
            Token::KwFn => f.write_str("fn"),
            Token::KwFor => f.write_str("for"),
            Token::KwIf => f.write_str("if"),
            Token::KwLet => f.write_str("let"),
            Token::KwLoop => f.write_str("loop"),
            Token::KwOverride => f.write_str("override"),
            Token::KwRequires => f.write_str("requires"),
            Token::KwReturn => f.write_str("return"),
            Token::KwStruct => f.write_str("struct"),
            Token::KwSwitch => f.write_str("switch"),
            Token::KwTrue => f.write_str("true"),
            Token::KwVar => f.write_str("var"),
            Token::KwWhile => f.write_str("while"),
            Token::Ident(s) => write!(f, "identifier `{s}`"),
            Token::ReservedWord(s) => write!(f, "reserved word `{s}`"),
            Token::AbstractInt(n) => write!(f, "{n}"),
            Token::AbstractFloat(n) => write!(f, "{n}"),
            Token::I32(n) => write!(f, "{n}i"),
            Token::U32(n) => write!(f, "{n}u"),
            Token::F32(n) => write!(f, "{n}f"),
            Token::F16(n) => write!(f, "{n}h"),
            Token::TemplateArgsStart => f.write_str("start of template"),
            Token::TemplateArgsEnd => f.write_str("end of template"),

            // extension: naga-ext
            Token::I64(n) => write!(f, "{n}li"),
            // extension: naga-ext
            Token::U64(n) => write!(f, "{n}lu"),
            // extension: naga-ext
            Token::F64(n) => write!(f, "{n}lf"),

            // extension: wesl-imports
            Token::SymColonColon => write!(f, "::"),
            // extension: wesl-imports
            Token::KwSelf => write!(f, "self"),
            // extension: wesl-imports
            Token::KwSuper => write!(f, "super"),
            // extension: wesl-imports
            Token::KwPackage => write!(f, "package"),
            // extension: wesl-imports
            Token::KwAs => write!(f, "as"),
            // extension: wesl-imports
            Token::KwImport => write!(f, "import"),
        }
    }
}

type Spanned<Tok, Loc, ParseError> = Result<(Loc, Tok, Loc), (Loc, ParseError, Loc)>;
type NextToken = Option<(Result<Token, ParseError>, Span)>;

#[derive(Clone)]
pub struct Lexer<'s> {
    source: &'s str,
    token_stream: SpannedIter<'s, Token>,
    next_token: NextToken,
    recognizing_template: bool,
    opened_templates: u32,
}

impl<'s> Lexer<'s> {
    pub fn new(source: &'s str) -> Self {
        let mut token_stream = Token::lexer_with_extras(source, LexerState::default()).spanned();
        let next_token =
            token_stream.find(|(tok, _)| tok.as_ref().is_ok_and(|tok| !tok.is_trivia()));

        Self {
            source,
            token_stream,
            next_token,
            recognizing_template: false,
            opened_templates: 0,
        }
    }

    fn take_two_tokens(&mut self) -> (NextToken, NextToken) {
        let mut tok1 = self.next_token.take();

        let lookahead = self.token_stream.extras.lookahead.take();
        let tok2 = match lookahead {
            Some(tok) => {
                let (_, span1) = tok1.as_mut().unwrap(); // safety: lookahead implies lexer looked at a `<` token
                let span2 = span1.start + 1..span1.end;
                Some((Ok(tok), span2))
            }
            None => self
                .token_stream
                .find(|(tok, _)| tok.as_ref().is_ok_and(|tok| !tok.is_trivia())),
        };

        (tok1, tok2)
    }

    fn next_token(&mut self) -> NextToken {
        let (cur, mut next) = self.take_two_tokens();

        let (cur_tok, cur_span) = match cur {
            Some((Ok(tok), span)) => (tok, span),
            Some((Err(e), span)) => return Some((Err(e), span)),
            None => return None,
        };

        if let Some((Ok(next_tok), next_span)) = &mut next {
            if (matches!(cur_tok, Token::Ident(_)) || cur_tok.is_keyword())
                && *next_tok == Token::SymLessThan
            {
                let source = &self.source[next_span.start..];
                if recognize_template_list(source) {
                    *next_tok = Token::TemplateArgsStart;
                    let cur_depth = self.token_stream.extras.depth;
                    self.token_stream.extras.template_depths.push(cur_depth);
                    self.opened_templates += 1;
                }
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
        Some((Ok(cur_tok), cur_span))
    }
}

/// Returns `true` if the source starts with a valid template list.
///
/// ## Specification
///
/// [3.9. Template Lists](https://www.w3.org/TR/WGSL/#template-lists-sec)
///
/// Contrary to the specification [template list discovery algorithm], this function also
/// checks that the template is syntactically valid (syntax: [*template_list*]).
///
/// [template list discovery algorigthm]: https://www.w3.org/TR/WGSL/#template-list-discovery
/// [*template_list*]: https://www.w3.org/TR/WGSL/#syntax-template_list
pub fn recognize_template_list(source: &str) -> bool {
    let mut lexer = Lexer::new(source);
    match lexer.next_token {
        Some((Ok(ref mut t), _)) if *t == Token::SymLessThan => *t = Token::TemplateArgsStart,
        _ => return false,
    };
    lexer.recognizing_template = true;
    lexer.opened_templates = 1;
    lexer.token_stream.extras.template_depths.push(0);
    crate::parser::recognize_template_list(lexer).is_ok()
}

#[test]
fn test_recognize_template() {
    // cases from the WGSL spec
    assert!(recognize_template_list("<i32,select(2,3,a>b)>"));
    assert!(!recognize_template_list("<d]>"));
    assert!(recognize_template_list("<B<<C>"));
    assert!(recognize_template_list("<B<=C>"));
    assert!(recognize_template_list("<(B>=C)>"));
    assert!(recognize_template_list("<(B!=C)>"));
    assert!(recognize_template_list("<(B==C)>"));
    // more cases
    assert!(recognize_template_list("<X>"));
    assert!(recognize_template_list("<X<Y>>"));
    assert!(recognize_template_list("<X<Y<Z>>>"));
    assert!(!recognize_template_list(""));
    assert!(!recognize_template_list(""));
    assert!(!recognize_template_list("<>"));
    assert!(!recognize_template_list("<b || c>d"));
}

pub trait TokenIterator: IntoIterator<Item = Spanned<Token, usize, ParseError>> {}

impl Iterator for Lexer<'_> {
    type Item = Spanned<Token, usize, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let tok = self.next_token();
        tok.map(|(tok, span)| match tok {
            Ok(tok) => Ok((span.start, tok, span.end)),
            Err(err) => Err((span.start, err, span.end)),
        })
    }
}

impl TokenIterator for Lexer<'_> {}
