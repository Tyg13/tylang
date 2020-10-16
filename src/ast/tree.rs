use crate::ast::{self, Expression, Item, Parse, Parser, Type, Visitor};
use crate::lexer::TokenKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub items: Vec<Item>,
}

impl Tree {
    pub fn valid(&self) -> bool {
        struct ErrorFinder {
            any_errors: bool,
        }
        impl Visitor for ErrorFinder {
            fn visit_item(&mut self, item: &Item) {
                self.any_errors = self.any_errors || item.is_error();
                ast::walk_item(self, item);
            }
            fn visit_type(&mut self, type_: &Type) {
                self.any_errors = self.any_errors || type_.is_error();
            }
            fn visit_expression(&mut self, expr: &Expression) {
                self.any_errors = self.any_errors || expr.is_error();
                ast::walk_expression(self, expr);
            }
        }
        let mut finder = ErrorFinder { any_errors: false };
        finder.visit_tree(self);
        !finder.any_errors
    }
}

impl Parse for Tree {
    fn parse(parser: &mut Parser) -> Option<Tree> {
        let mut items = Vec::new();
        while parser.peek().is_some() {
            let item = parser.parse_one().unwrap_or_else(|| {
                parser.advance_past_one_of(&[TokenKind::RightBrace, TokenKind::SemiColon]);
                Item::error()
            });
            items.push(item);
        }
        Some(Tree { items })
    }
}

#[cfg(test)]
pub mod test {
    use crate::ast::*;

    pub fn tree(items: &[Item]) -> Tree {
        Tree {
            items: items.to_vec(),
        }
    }

    #[test]
    fn empty() {
        assert_eq!(test::parse(""), tree(&[]))
    }
}
