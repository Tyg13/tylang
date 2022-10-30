use crate::{Anchor, Span};

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
    pub fn file<S: AsRef<str>>(mut self, file: S) -> Self {
        self.file = Some(file.as_ref().to_string());
        self
    }
    pub fn source<S: AsRef<str>>(mut self, input: S) -> Self {
        self.source = Some(input.as_ref().to_string());
        self
    }
    pub fn build(self) -> Source {
        let file = self
            .file
            .map(|s| if s == "-" { "<stdin>".to_string() } else { s })
            .unwrap_or(String::from("<err>"));
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

    pub fn char(&self, pos: usize) -> Option<&char> {
        self.chars.get(pos)
    }

    /// Given an offset, find the anchor (line, column) that corresponds to that offset in the
    /// source file.
    pub fn anchor_at(&self, offset: usize) -> Option<Anchor> {
        // We start by performing a binary search on the line end array.
        match self.line_ends.binary_search(&offset) {
            // We're in between two line ends -- idx is the
            Err(idx) => {
                if idx < self.line_ends.len() {
                    let line_start = if idx == 0 {
                        0
                    } else {
                        self.line_ends[idx - 1] + 1
                    };
                    let column = offset - line_start;
                    Some(Anchor {
                        // Convert between 0- and 1-based indexing
                        line: idx + 1,
                        column: column + 1,
                        offset,
                    })
                } else {
                    // Out of bounds
                    None
                }
            }
            // Our offset is at the end of a line
            Ok(idx) => Some(Anchor {
                line: idx + 1,
                column: 0,
                offset,
            }),
        }
    }

    pub fn span_for(&self, range: std::ops::Range<usize>) -> Option<Span> {
        match (self.anchor_at(range.start), self.anchor_at(range.end)) {
            (Some(start), Some(end)) => Some(Span { start, end }),
            _ => None,
        }
    }

    pub fn line(&self, number: usize) -> Option<String> {
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
        Some(format!(
            "{prefix}{context}\n{arm}{hand}{trailer}",
            arm = str::repeat("-", prefix.len() + start - 1),
        ))
    }

    pub fn give_context_span(
        &self,
        span: Span,
        arm: HandPosition,
    ) -> Option<String> {
        self.give_context_span_and_label(span, arm, None)
    }
}
