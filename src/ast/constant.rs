use crate::ast::{Expression, ExpressionKind, Parse, Parser};
use crate::lexer::TokenKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Constant {
    pub value: usize,
    pub parity: Parity,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Parity {
    Unsigned,
    Signed,
}

impl std::fmt::Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Constant({value})", value = self.value)
    }
}

impl Constant {
    pub(super) fn new<P: Into<Parity>>(value: usize, parity: P) -> Self {
        Self {
            value,
            parity: parity.into(),
        }
    }
}

impl From<Constant> for Expression {
    fn from(cons: Constant) -> Self {
        Self {
            kind: ExpressionKind::Constant(cons),
        }
    }
}

impl From<crate::lexer::Parity> for Parity {
    fn from(parity: crate::lexer::Parity) -> Self {
        match parity {
            crate::lexer::Parity::Signed => Parity::Signed,
            crate::lexer::Parity::Unsigned => Parity::Unsigned,
        }
    }
}

impl Parse for Constant {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let number = parser
            .expect(TokenKind::Number)
            .map(|token| token.as_num().unwrap())?;
        Some(Constant::new(number.value, number.parity))
    }
}

#[cfg(test)]
mod test {
    use crate::ast::*;

    fn unsigned(value: usize) -> Constant {
        Constant {
            value,
            parity: Parity::Unsigned,
        }
    }

    fn signed(value: usize) -> Constant {
        Constant {
            value,
            parity: Parity::Signed,
        }
    }

    #[test]
    fn unsigned_num() {
        assert_eq!(unsigned(10), test::parse_one("10"));
    }

    #[test]
    fn signed_num() {
        assert_eq!(signed(10), test::parse_one("-10"));
    }
}
