use std::ops::Range;

use derive_more::derive::{AsMut, AsRef, Deref, DerefMut, From};
use syntax::TextRange;

pub type Id = u32;

#[cfg_attr(feature = "tokrepr", derive(tokrepr::TokRepr))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// The lower bound of the span (inclusive).
    pub start: usize,
    /// The upper bound of the span (exclusive).
    pub end: usize,
}
// Deref, DerefMut, AsRef, AsMut
impl Span {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
    pub fn extend(&self, other: Span) -> Self {
        Self {
            start: self.start,
            end: other.end,
        }
    }
}

impl From<Range<usize>> for Span {
    fn from(value: Range<usize>) -> Self {
        Span::new(value)
    }
}

impl From<TextRange> for Span {
    fn from(value: TextRange) -> Self {
        let start = usize::from(value.start());
        let end = usize::from(value.end());
        Span { start, end }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Clone, Debug, Deref, DerefMut, AsRef, AsMut, From)]
pub struct Spanned<T> {
    span: Span,
    #[deref(forward)]
    #[deref_mut(forward)]
    #[as_ref(T)]
    #[as_mut(T)]
    #[from(T)]
    node: Box<T>,
}

// we ignore the spans for equality comparison
impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.node.eq(&other.node)
    }
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self::new_boxed(Box::new(node), span)
    }
    pub fn new_boxed(node: Box<T>, span: Span) -> Self {
        Self { span, node }
    }
    pub fn span(&self) -> Span {
        self.span
    }
    pub fn node(&self) -> &T {
        self
    }
    pub fn node_mut(&mut self) -> &mut T {
        self
    }
    pub fn into_inner(self) -> T {
        *self.node
    }
}

impl<T> From<T> for Spanned<T> {
    fn from(value: T) -> Self {
        Self {
            span: Default::default(),
            node: Box::new(value),
        }
    }
}
