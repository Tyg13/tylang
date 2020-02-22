#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: Anchor,
    pub end: Anchor,
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

#[macro_export]
macro_rules! span {
    ($start_line:literal : $start_column:literal, $end_line:literal:$end_column:literal) => {
        span! {
            $crate::util::Anchor {
                line: $start_line,
                column: $start_column,
            },
            $crate::util::Anchor {
                line: $end_line,
                column: $end_column,
            }
        }
    };
    ($start:expr, $end:expr) => {
        $crate::util::Span {
            start: $start,
            end: $end,
        }
    };
}

pub struct Source {
    file: String,
    lines: Vec<String>,
}

pub struct SourceBuilder {
    file: Option<String>,
    lines: Vec<String>,
}

impl SourceBuilder {
    pub fn new() -> Self {
        Self {
            file: None,
            lines: vec![],
        }
    }
    pub fn file<S: AsRef<str>>(mut self, file: S) -> Self {
        self.file = Some(String::from(file.as_ref()));
        self
    }
    pub fn lines<S: AsRef<str>>(mut self, input: S) -> Self {
        self.lines = input.as_ref().lines().map(String::from).collect();
        self
    }
    pub fn build(self) -> Source {
        let file = self
            .file
            .map(|s| if s == "-" { String::from("<stdin>") } else { s })
            .unwrap_or(String::from("<err>"));
        let lines = self.lines;
        Source { file, lines }
    }
}

impl Source {
    pub fn lines(&self) -> &Vec<String> {
        &self.lines
    }

    pub fn line(&self, number: usize) -> Option<String> {
        self.lines.get(number - 1).map(|s| s.clone())
    }

    pub fn file(&self) -> String {
        self.file.clone()
    }
}
