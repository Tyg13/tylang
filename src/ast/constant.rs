use super::*;
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

impl Parser {
    pub(super) fn parse_constant(&self, cursor: &mut Cursor) -> Option<Constant> {
        let token = cursor.expect_token(TokenKind::Number);
        let number = self.map.num(&token)?;
        Some(Constant {
            value: number.value,
            parity: number.parity.into(),
        })
    }
}

#[cfg(tests)]
mod tests {
    #[macro_export]
    macro_rules! con {
        ($span:expr, $value:expr) => {
            $crate::ast::Constant { value: $value }
        };
    }
}
