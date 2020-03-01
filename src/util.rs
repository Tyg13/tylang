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
        $crate::util::Span {
            start: $crate::util::Anchor {
                line: $start_line,
                column: $start_column,
            },
            end: $crate::util::Anchor {
                line: $end_line,
                column: $end_column,
            },
        }
    };
    ($start:expr, $end:expr) => {
        $crate::util::Span {
            start: $start.span.start,
            end: $end.span.end,
        }
    };
}

pub struct Source {
    file: String,
    lines: Vec<String>,
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
        self.file = Some(String::from(file.as_ref()));
        self
    }
    pub fn source<S: AsRef<str>>(mut self, input: S) -> Self {
        self.source = Some(String::from(input.as_ref()));
        self
    }
    pub fn build(self) -> Source {
        let file = self
            .file
            .map(|s| if s == "-" { String::from("<stdin>") } else { s })
            .unwrap_or(String::from("<err>"));
        let lines = self
            .source
            .unwrap_or(String::new())
            .lines()
            .map(|line| line.to_string())
            .collect();
        Source { file, lines }
    }
}

pub enum ArmPosition {
    Begin,
    End,
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

    pub fn give_context(&self, span: Span, arm: ArmPosition) -> Option<String> {
        const PADDING_LEN: usize = 4;
        let context = self.line(span.end.line)?;
        let prefix = format!(
            "[{file}:{line}:{column}]{padding}",
            file = self.file,
            line = span.start.line,
            column = span.start.column,
            padding = str::repeat(" ", PADDING_LEN)
        );
        let column = match arm {
            ArmPosition::Begin => span.start.column,
            ArmPosition::End => span.end.column,
        };
        Some(format!(
            "{prefix}{context}\n{arm}{hand}",
            prefix = prefix,
            context = context,
            arm = str::repeat("-", prefix.len() + column - 1),
            hand = str::repeat(
                "^",
                std::cmp::max(1, span.end.column.saturating_sub(column))
            ),
        ))
    }
}
