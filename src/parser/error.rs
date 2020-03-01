use super::*;

#[derive(Clone, PartialEq, Debug)]
pub(super) enum Error {
    UnexpectedToken(String),
    Internal(String),
    EOF,
}

impl Parser<'_> {
    pub(super) fn report_err(&mut self, err: Error) {
        || -> std::io::Result<()> {
            write!(self.out, "\nParseError: ")?;
            match err {
                Error::EOF => writeln!(self.out, "unexpected EOF")?,
                Error::Internal(ref message) => writeln!(self.out, "internal: {}", message)?,
                Error::UnexpectedToken(ref expected) => {
                    writeln!(self.out, "expected `{}`", expected)?
                }
            }
            if let Some(token) = self.peek().ok() {
                if let Some(context) = self.source.give_context(
                    token.span,
                    match err {
                        Error::EOF => ArmPosition::End,
                        _ => ArmPosition::Begin,
                    },
                ) {
                    writeln!(self.out, "{}", context)?;
                }
            }
            Ok(())
        }()
        .expect("couldn't write to parser out");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tree;

    macro_rules! assert_lines {
        ($expected:literal, $actual:expr) => {
            for (actual, expected) in $actual.lines().zip($expected.lines()) {
                assert_eq!(actual, expected);
            }
        };
    }

    #[test]
    fn expected() {
        let (_, stdout) = tree![
            "let=;",
            token      { Let,       span!(1:01, 1:04) },
            token      { Equals,    span!(1:04, 1:05) },
            token      { SemiColon, span!(1:05, 1:06) },
        ];
        assert_lines!(
            r#"
ParseError: expected `identifier`
[<err>:1:4]    let=;
------------------^
"#,
            &stdout
        );
        let (_, stdout) = tree![
            "let n =;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "n",       span!(1:05, 1:06) },
            token      { Equals,    span!(1:07, 1:08) },
            token      { SemiColon, span!(1:08, 1:09) },
        ];
        assert_lines!(
            r#"
ParseError: expected `expression`
[<err>:1:8]    let n =;
----------------------^
"#,
            &stdout
        );
    }

    #[test]
    fn multiple() {
        let (_, stdout) = tree![
            "let=;\nlet n =;",
            token      { Let,       span!(1:01, 1:04) },
            token      { Equals,    span!(1:04, 1:05) },
            token      { SemiColon, span!(1:05, 1:06) },
            token      { Let,       span!(2:01, 2:04) },
            identifier { "n",       span!(2:05, 2:06) },
            token      { Equals,    span!(2:07, 2:08) },
            token      { SemiColon, span!(2:08, 2:09) },
        ];
        assert_lines!(
            r#"
ParseError: expected `identifier`
[<err>:1:4]    let=;
------------------^

ParseError: expected `expression`
[<err>:2:8]    let n =;
----------------------^
"#,
            &stdout
        );
    }
}
