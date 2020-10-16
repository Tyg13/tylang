use super::*;
use crate::lexer::{Token, TokenKind};
use crate::util::{HandPosition, Source, Span};
use std::io::Write;

pub trait ErrorHandler {
    fn issue_diagnostic(&mut self, error: &Error);
}
pub struct StreamHandler<'out, 'source> {
    pub(super) source: &'source Source,
    pub(super) out: &'out mut dyn Write,
}

impl<'out, 'source> StreamHandler<'out, 'source> {
    pub fn new(source: &'source Source, out: &'out mut dyn Write) -> Self {
        Self { source, out }
    }
}

impl ErrorHandler for StreamHandler<'_, '_> {
    fn issue_diagnostic(&mut self, error: &Error) {
        || -> std::io::Result<()> {
            writeln!(self.out, "{}", error)?;
            if let Some(context) = error
                .span
                .and_then(|span| self.source.give_context_span(span, HandPosition::WholeSpan))
            {
                writeln!(self.out, "{}", context)?;
            }
            Ok(())
        }()
        .expect("Couldn't write to stream!");
    }
}

pub struct Error {
    span: Option<Span>,
    kind: ErrorKind,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::Generic(message) => message.fmt(f),
        }
    }
}

pub enum ErrorKind {
    Generic(String),
}

pub struct ExpectedError {
    expected: TokenKind,
    actual: Option<TokenKind>,
}

impl ExpectedError {
    pub fn new(expected: TokenKind, token: Option<Token>) -> Self {
        Self {
            expected,
            actual: token.map(|token| token.kind()),
        }
    }
}

impl std::fmt::Display for ExpectedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Expected `{}`", self.expected)?;
        if let Some(actual) = self.actual {
            write!(f, ", got `{}`", actual)?;
        }
        Ok(())
    }
}

impl Parser<'_, '_> {
    pub fn issue_diagnostic<S: AsRef<str>>(&mut self, message: S) {
        // TODO: should pass context, not derive it from current state
        let message = message.as_ref().to_string();
        let span = self.peek().map(|token| token.span());
        self.error_handler.issue_diagnostic(&Error {
            span,
            kind: ErrorKind::Generic(message),
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! assert_error {
        ($input:literal, $error:literal$(,)?) => {{
            let mut out = Vec::new();
            Parser::test_with_log($input, &mut out).parse();
            assert_eq!(String::from_utf8(out).unwrap(), $error);
        }};
    }

    #[test]
    fn expected_identifier() {
        assert_error!(
            "fn a() -> {}",
            "Expected `identifier`, got `{`\n\
            [<err>:1:11] | fn a() -> {}\n\
            -------------------------^\n"
        );
        assert_error!(
            "fn ();",
            "Expected `identifier`, got `(`\n\
            [<err>:1:4] | fn ();\n\
            -----------------^\n"
        )
    }
}
