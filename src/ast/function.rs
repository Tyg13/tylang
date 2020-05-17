use super::*;
use crate::lexer::{TokenKind, TokenTree, TreeKind};

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub identifier: String,
    pub parameters: Vec<Parameter>,
    pub body: Option<Scope>,
    pub return_type: Type,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub variable: Variable,
    pub type_: Type,
}

impl Parser {
    pub(super) fn parse_function(&self, cursor: &mut Cursor<'_>) -> Function {
        cursor.expect_token(TokenKind::Fn);
        let identifier = self
            .map
            .ident(&cursor.expect_token(TokenKind::Identifier))
            .cloned()
            .unwrap();
        let parameters = {
            let tree = cursor.expect_tree(TreeKind::Parens);
            let mut cursor = Cursor::new(&tree.children);
            self.parse_parameters(&mut cursor)
        };
        cursor.expect_token(TokenKind::Arrow);
        let return_type = self
            .parse_type(cursor)
            .expect("Unable to parse return type in function.");
        let body = if let Some(tree) = cursor.maybe_tree(TreeKind::Braces) {
            let mut cursor = Cursor::new(&tree.children);
            Some(self.parse_scope(&mut cursor))
        } else {
            cursor.expect_token(TokenKind::SemiColon);
            None
        };
        Function {
            identifier,
            parameters,
            body,
            return_type,
        }
    }

    fn parse_parameters(&self, cursor: &mut Cursor) -> Vec<Parameter> {
        let mut parameters = Vec::new();
        while let Some(tree) = cursor.peek() {
            match tree {
                TokenTree::Tree(tree) => {
                    panic!("Unexpected {} in parameter list", tree.kind);
                }
                _ => {
                    let variable = self
                        .parse_variable(cursor)
                        .expect("Couldn't parse variable in parameter list");
                    cursor.expect_token(TokenKind::Colon);
                    let type_ = self
                        .parse_type(cursor)
                        .expect("Couldn't parse type in parameter list");
                    parameters.push(Parameter { variable, type_ });
                    cursor.maybe_token(TokenKind::Comma);
                }
            }
        }
        parameters
    }
}
