use crate::{Anchor, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (line, column, offset) = (self.line, self.column, self.offset);
        write!(f, "[{line}:{column}@{offset}]")
    }
}

pub struct Source {
    file: String,
    chars: Vec<char>,
    line_ends: Vec<usize>,
}

pub struct SourceBuilder {
    file: Option<String>,
    source: Option<String>,
}

impl SourceBuilder {
    pub fn new() -> Self {
        Self {
            file: None,
            source: None,
        }
    }
    pub fn file<S: Into<String>>(mut self, file: S) -> Self {
        self.file = Some(file.into());
        self
    }
    pub fn source<S: Into<String>>(mut self, input: S) -> Self {
        self.source = Some(input.into());
        self
    }
    pub fn build(self) -> Source {
        let file = self.file.map_or("<err>".to_string(), |s| {
            if s == "-" { "<stdin>".to_string() } else { s }
        });
        let chars: Vec<char> =
            self.source.unwrap_or_default().chars().collect();
        let line_ends = chars
            .iter()
            .enumerate()
            .filter(|(_, &b)| b == '\n')
            .map(|(offset, _)| offset)
            .collect();
        Source {
            file,
            chars,
            line_ends,
        }
    }
}

pub enum HandPosition {
    BeginOfSpan,
    WholeSpan,
    EndOfSpan,
}

impl Source {
    pub fn read_path(path: &str) -> Self {
        SourceBuilder::new()
            .file(path)
            .source(std::fs::read_to_string(path).unwrap())
            .build()
    }

    pub fn from_str(s: &str) -> Self {
        SourceBuilder::new().source(s).build()
    }

    /// Given an offset, find the anchor (line, column) that corresponds to that offset in the
    /// source file.
    pub fn anchor_at(&self, offset: usize) -> Option<Anchor> {
        let column = |line: usize, offset: usize| {
            let line_start = if line == 0 {
                // First line -- start is offset 0.
                0
            } else {
                self.line_ends[line - 1] + 1
            };
            offset - line_start
        };

        // We start by performing a binary search on the line end array.
        match self.line_ends.binary_search(&offset) {
            // We're in between two line ends -- idx is the line number of this line.
            Err(line) => {
                if line >= self.line_ends.len() {
                    // Out of bounds
                    return None;
                }
                Some(Anchor {
                    // Convert between 0- and 1-based indexing
                    line: line + 1,
                    column: column(line, offset) + 1,
                    offset,
                })
            }
            // Our offset is at the end of a line
            Ok(line) => {
                // Convert between 0- and 1-based indexing
                Some(Anchor {
                    line: line + 1,
                    column: column(line, offset) + 1,
                    offset,
                })
            }
        }
    }

    pub fn span_for(&self, range: std::ops::Range<usize>) -> Option<Span> {
        match (self.anchor_at(range.start), self.anchor_at(range.end)) {
            (Some(start), Some(end)) => Some(Span { start, end }),
            _ => None,
        }
    }

    pub fn num_lines(&self) -> usize {
        let last_line_has_no_line_end =
            self.chars.last().filter(|c| **c != '\n').is_some();
        self.line_ends.len() + last_line_has_no_line_end as usize
    }

    pub fn line(&self, number: usize) -> Option<String> {
        if number > self.num_lines() {
            return None;
        }
        let index = number.checked_sub(1)?;
        let prev = index.checked_sub(1);
        let begin = match prev {
            // If we're not on the first line, this line begins at one past the index of the
            // previous EOL.
            Some(prev) => *self.line_ends.get(prev)? + 1,
            // If we're on the first line, there is no previous line, use the index of the
            // beginning of the file instead.
            None => 0,
        };
        let end = match self.line_ends.get(index) {
            // Get this line's EOL, if it exists.
            Some(val) => *val,
            // If we're asking for EOL and it doesn't exist, this is the last line in a file that
            // file doesn't end in '\n'. Use the index of EOF instead.
            None => self.chars.len(),
        };
        let line = self.chars[begin..end].iter().collect();
        Some(line)
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn give_context_at(&self, at: Anchor) -> Option<String> {
        self.give_context_span(
            Span { start: at, end: at },
            HandPosition::BeginOfSpan,
        )
    }

    pub fn give_context_span_and_label(
        &self,
        span: Span,
        arm: HandPosition,
        label: Option<&str>,
    ) -> Option<String> {
        use HandPosition::*;
        let pos = match arm {
            BeginOfSpan | WholeSpan => span.start,
            EndOfSpan => span.end,
        };
        let context = self.line(pos.line)?;
        let prefix = format!(
            "[{file}:{line}:{column}] | ",
            file = self.file,
            line = pos.line,
            column = pos.column,
        );
        let (start, end) = match arm {
            BeginOfSpan => (span.start.column, span.start.column),
            WholeSpan => (span.start.column, span.end.column),
            EndOfSpan => (span.end.column, span.end.column),
        };
        let arm = str::repeat("-", prefix.len() + start - 1);
        let hand =
            str::repeat("^", std::cmp::max(1, end.saturating_sub(start)));
        let trailer = if let Some(label) = label {
            format!(
                "\n{spaces}|--- {label}",
                spaces =
                    str::repeat(" ", prefix.len() + hand.len() / 2 + start - 1)
            )
        } else {
            "".to_string()
        };
        Some(format!("{prefix}{context}\n{arm}{hand}{trailer}"))
    }

    pub fn give_context_span(
        &self,
        span: Span,
        arm: HandPosition,
    ) -> Option<String> {
        self.give_context_span_and_label(span, arm, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default() {
        let s = SourceBuilder::new().build();
        assert_eq!(s.file, "<err>");
        assert_eq!(s.chars, Vec::new());
        assert_eq!(s.line_ends, Vec::new());
    }

    #[test]
    fn build_with_file() {
        let s = SourceBuilder::new().file("foo").build();
        assert_eq!(s.file, "foo");
    }

    #[test]
    fn build_with_stdin_file() {
        let s = SourceBuilder::new().file("-").build();
        assert_eq!(s.file, "<stdin>");
    }

    #[test]
    fn build_with_one_line_source() {
        let s = SourceBuilder::new().source("foo\n").build();
        assert_eq!(s.chars, vec!['f', 'o', 'o', '\n']);
        assert_eq!(s.line_ends, vec![3]);
    }

    #[test]
    fn build_with_multi_line_source() {
        let s = SourceBuilder::new().source("foo\nb\n  baz\n").build();
        assert_eq!(
            s.chars,
            vec![
                'f', 'o', 'o', '\n', 'b', '\n', ' ', ' ', 'b', 'a', 'z', '\n'
            ]
        );
        assert_eq!(s.line_ends, vec![3, 5, 11]);
    }

    #[test]
    fn anchor_at_zero() {
        let a = Source::from_str("lorem\n").anchor_at(0).unwrap();
        assert_eq!(a.line, 1);
        assert_eq!(a.column, 1);
        assert_eq!(a.offset, 0);
    }

    #[test]
    fn anchor_in_first_line() {
        let a = Source::from_str("lorem\n").anchor_at(4).unwrap();
        assert_eq!(a.line, 1);
        assert_eq!(a.column, 5);
        assert_eq!(a.offset, 4);
    }

    #[test]
    fn anchor_at_end_of_first_line() {
        let a = Source::from_str("lorem\nipsum\n").anchor_at(5).unwrap();
        assert_eq!(a.line, 1);
        assert_eq!(a.column, 6);
        assert_eq!(a.offset, 5);
    }

    #[test]
    fn anchor_in_empty_line() {
        let a = Source::from_str("lorem\n\nipsum\n").anchor_at(6).unwrap();
        assert_eq!(a.line, 2);
        assert_eq!(a.column, 1);
        assert_eq!(a.offset, 6);
    }

    #[test]
    fn anchor_in_nth_line() {
        let a = Source::from_str("lorem\nipsum\n").anchor_at(7).unwrap();
        assert_eq!(a.line, 2);
        assert_eq!(a.column, 2);
        assert_eq!(a.offset, 7);
    }

    #[test]
    fn anchor_at_end_of_nth_line() {
        let a = Source::from_str("lorem\nipsum\n").anchor_at(11).unwrap();
        assert_eq!(a.line, 2);
        assert_eq!(a.column, 6);
        assert_eq!(a.offset, 11);
    }

    #[test]
    fn anchor_out_of_bounds() {
        let a = Source::from_str("lorem\n\nipsum\n").anchor_at(20);
        assert_eq!(a, None);
    }

    #[test]
    fn context_with_file_name() {
        let s = SourceBuilder::new()
            .file("foo")
            .source("lorem ipsum\n")
            .build();
        let ctx = s
            .give_context_at(Anchor {
                line: 1,
                column: 1,
                offset: 0,
            })
            .unwrap();
        assert_eq!(
            ctx,
            "[foo:1:1] | lorem ipsum\n\
             ------------^"
        );
    }

    #[test]
    fn context_on_nth_line_and_column() {
        let s = SourceBuilder::new()
            .source("lorem ipsum\nsit dolor amit")
            .build();
        let ctx = s
            .give_context_at(Anchor {
                line: 2,
                column: 4,
                offset: 15,
            })
            .unwrap();
        assert_eq!(
            ctx,
            "[<err>:2:4] | sit dolor amit\n\
             -----------------^"
        );
    }

    #[test]
    fn get_first_line() {
        assert_eq!(Source::from_str("foo\n").line(1).as_deref(), Some("foo"));
        assert_eq!(Source::from_str("foo").line(1).as_deref(), Some("foo"));
        assert_eq!(
            Source::from_str("foo\nbar\n").line(1).as_deref(),
            Some("foo")
        );
        assert_eq!(Source::from_str("\nbar\n").line(1).as_deref(), Some(""));
    }

    #[test]
    fn get_nth_line() {
        assert_eq!(Source::from_str("foo\nbar\nbaz\n").line(2).unwrap(), "bar");
        assert_eq!(Source::from_str("foo\nbar\nbaz").line(2).unwrap(), "bar");
        assert_eq!(Source::from_str("\nbaz\n\n").line(2).unwrap(), "baz");
        assert_eq!(Source::from_str("foo\nbar\n\nbaz").line(3).unwrap(), "");
    }

    #[test]
    fn get_last_line() {
        assert_eq!(Source::from_str("foo\nbar\n").line(2).unwrap(), "bar");
        assert_eq!(Source::from_str("foo\nbaz").line(2).unwrap(), "baz");
        assert_eq!(Source::from_str("\nbaz\n").line(2).unwrap(), "baz");
        assert_eq!(Source::from_str("\nfoo\n\n").line(3).unwrap(), "");
    }

    #[test]
    fn get_line_out_of_bounds() {
        assert_eq!(Source::from_str("").line(0), None);
        assert_eq!(Source::from_str("foo\n").line(0), None);
        assert_eq!(Source::from_str("").line(2), None);
        assert_eq!(Source::from_str("foo\n").line(2), None);
        assert_eq!(Source::from_str("foo").line(2), None);
    }

    #[test]
    fn num_lines() {
        assert_eq!(Source::from_str("").num_lines(), 0);
        assert_eq!(Source::from_str("foo").num_lines(), 1);
        assert_eq!(Source::from_str("foo\n").num_lines(), 1);
        assert_eq!(Source::from_str("foo\nbar").num_lines(), 2);
        assert_eq!(Source::from_str("foo\nbar\n").num_lines(), 2);
    }
}
