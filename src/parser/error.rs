use super::*;

#[derive(Clone, PartialEq, Debug)]
pub enum Error {
    UnexpectedToken { expected: String, unexpected: Token },
    Internal(String),
    EOF,
}

impl Parser<'_> {
    pub fn report_err(&mut self, err: Error) {
        || -> std::io::Result<()> {
            write!(self.out, "\nParseError: ")?;
            match err {
                Error::EOF => writeln!(self.out, "unexpected EOF")?,
                Error::Internal(ref message) => writeln!(self.out, "internal: {}", message)?,
                Error::UnexpectedToken {
                    ref expected,
                    unexpected,
                } => writeln!(
                    self.out,
                    "expected `{}`, got `{}`",
                    expected, unexpected.kind
                )?,
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
    use crate::{token, tree};

    #[test]
    fn expected() {
        let (_, stdout) = tree![
            "let=;",
            token      { Let,       span!(1:01, 1:04) },
            token      { Assign,    span!(1:04, 1:05) },
            token      { SemiColon, span!(1:05, 1:06) },
        ];
        assert_eq!(
            r#"
ParseError: expected `identifier`, got `=`
[<err>:1:4]    let=;
------------------^
"#,
            &stdout
        );
    }

    #[test]
    fn multiple() {
        let (_, stdout) = tree![
            "let=;",
            token      { Let,       span!(1:01, 1:04) },
            token      { Assign,    span!(1:04, 1:05) },
            token      { SemiColon, span!(1:05, 1:06) },
        ];
        assert_eq!(
            r#"
ParseError: expected `identifier`, got `=`
[<err>:1:4]    let=;
------------------^
"#,
            &stdout
        );
    }
}
