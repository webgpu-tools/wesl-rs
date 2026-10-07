//! Renders syntax nodes and records where each spanned node ends up in the text.

use std::{fmt, ops::Range};

use pretty::{Render, RenderAnnotated};

use crate::{
    span::Span,
    syntax::TranslationUnit,
    syntax_pretty::{Alloc, ToDoc},
};

/// A spanned syntax node and the range of rendered text it produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrintedSpan {
    /// The number of spanned nodes enclosing this one.
    pub depth: usize,
    /// The byte range of the node in the rendered text, without surrounding whitespace.
    pub range: Range<usize>,
    /// The span of the node in the source it was parsed from.
    pub source_span: Span,
}

/// A [`Render`] target that collects the text and the ranges of the annotated documents.
#[derive(Default)]
struct Recorder {
    /// The spans of the nodes that are being rendered, with the position of their first byte.
    open: Vec<(Span, usize)>,
    /// The recorded nodes, ordered by when they finished rendering.
    spans: Vec<PrintedSpan>,
    /// The text rendered so far.
    text: String,
}

impl Render for Recorder {
    type Error = fmt::Error;

    fn write_str(&mut self, s: &str) -> Result<usize, fmt::Error> {
        self.text.push_str(s);
        Ok(s.len())
    }

    fn fail_doc(&self) -> fmt::Error {
        fmt::Error
    }
}

impl<'a> RenderAnnotated<'a, Span> for Recorder {
    fn push_annotation(&mut self, span: &'a Span) -> Result<(), fmt::Error> {
        self.open.push((*span, self.text.len()));
        Ok(())
    }

    fn pop_annotation(&mut self) -> Result<(), fmt::Error> {
        let (span, start) = self.open.pop().ok_or(fmt::Error)?;
        if start < self.text.len() {
            self.spans.push(PrintedSpan {
                depth: self.open.len(),
                range: start..self.text.len(),
                source_span: span,
            });
        }
        Ok(())
    }
}

/// Renders `node` and returns the text with the range of every spanned node in it.
///
/// The text is the same as the one of [`Display`](std::fmt::Display).
pub fn print_with_spans<T: ToDoc + ?Sized>(node: &T) -> (String, Vec<PrintedSpan>) {
    let arena = Alloc::new();
    let mut recorder = Recorder::default();
    node.to_doc(&arena)
        .into_doc()
        .render_raw(usize::MAX, &mut recorder)
        .expect("rendering to a string cannot fail");
    (recorder.text, recorder.spans)
}

impl TranslationUnit {
    /// Renders the module and returns the text with the range of every spanned node in it.
    pub fn print_with_spans(&self) -> (String, Vec<PrintedSpan>) {
        print_with_spans(self)
    }
}
