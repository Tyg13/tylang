mod anchor;
mod span;
pub use anchor::*;
pub use span::*;

pub mod intern_map;
pub mod sparse_matrix;
pub mod vec_graph;

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
        let chars: Vec<char> = self.source.unwrap_or_default().chars().collect();
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
    pub fn char(&self, pos: usize) -> Option<&char> {
        self.chars.get(pos)
    }

    pub fn line(&self, number: usize) -> Option<String> {
        let index = number.checked_sub(1)?;
        let begin = match index.checked_sub(1) {
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
        self.give_context_span(Span { start: at, end: at }, HandPosition::BeginOfSpan)
    }

    pub fn give_context_span(&self, span: Span, arm: HandPosition) -> Option<String> {
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
        Some(format!(
            "{prefix}{context}\n{arm}{hand}",
            arm = str::repeat("-", prefix.len() + start - 1),
            hand = str::repeat("^", std::cmp::max(1, end.saturating_sub(start))),
        ))
    }
}
