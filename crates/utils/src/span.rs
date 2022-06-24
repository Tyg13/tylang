use crate::Anchor;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Span {
    pub start: Anchor,
    pub end: Anchor,
}

impl Span {
    pub fn size(&self) -> usize {
        self.end.offset - self.start.offset
    }
}

use std::fmt;
impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{start_line}:{start_column}, {end_line}:{end_column}]",
            start_line = self.start.line,
            start_column = self.start.column,
            end_line = self.end.line,
            end_column = self.end.column,
        )
    }
}

impl Span {
    pub fn contains(&self, other: &Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}
