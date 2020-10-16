use crate::ast::{Function, Parse, Parser};
use crate::lexer::TokenKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
}

impl Item {
    pub fn error() -> Self {
        Self {
            kind: ItemKind::Error,
        }
    }

    pub fn is_error(&self) -> bool {
        self.kind == ItemKind::Error
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    Function(Function),
    Error,
}

impl Parse for Item {
    fn parse(parser: &mut Parser) -> Option<Item> {
        parser.some_or_backtrack(|parser| match parser.peek()?.kind() {
            TokenKind::Fn => parser.parse_one::<Function>().map(Item::from),
            _ => None,
        })
    }
}

#[cfg(test)]
pub mod test {
    use crate::ast::*;

    pub fn item<I: Into<Item>>(item: I) -> Item {
        item.into()
    }
}
