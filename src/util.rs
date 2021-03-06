pub mod intern_map;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anchor {
    pub line: usize,
    pub column: usize,
}

impl std::ops::Add<usize> for Anchor {
    type Output = Anchor;
    fn add(self, rhs: usize) -> Self::Output {
        Self {
            line: self.line + rhs,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Span {
    pub start: Anchor,
    pub end: Anchor,
}

#[macro_export]
macro_rules! span {
    ($start_line:expr;$start_column:expr, $end_line:expr;$end_column:literal) => {
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
        let chars: Vec<char> = self.source.unwrap_or(String::new()).chars().collect();
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

pub enum ArmPosition {
    Begin,
    End,
}

impl Source {
    pub fn char(&self, pos: usize) -> Option<&char> {
        self.chars.get(pos)
    }

    pub fn line(&self, number: usize) -> Option<String> {
        if number == 0 {
            return None;
        }
        let index = number - 1;
        // Start this line at the previous EOL. If we're on the first line,
        // there is no previous line. Use the beginning of the file instead.
        let begin = match index.checked_sub(1) {
            Some(prev) => *self.line_ends.get(prev)? + 1,
            None => 0,
        };
        // If we're asking for EOL and the file doesn't end in '\n', use EOF.
        let end = match self.line_ends.get(index) {
            Some(val) => *val,
            None => self.chars.len(),
        };
        let line = self.chars[begin..end].iter().collect();
        Some(line)
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
