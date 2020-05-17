use crate::ast::{Function, Tree};
use crate::lexer::{Token, TokenKind, TokenMap, TokenTree, Tree as SubTree, TreeKind};

pub fn parse(map: TokenMap) -> Tree {
    Parser::new(map).parse()
}

pub struct Parser {
    pub map: TokenMap,
}

impl Parser {
    pub fn new(map: TokenMap) -> Self {
        Self { map }
    }

    pub fn parse(&self) -> Tree {
        let mut functions = Vec::new();
        let tree = self.map.tree.tree().unwrap();
        let children = &tree.children;
        let mut cursor = Cursor::new(children);
        while let Some(tree) = cursor.peek() {
            match tree {
                TokenTree::Tree(tree) => panic!("Unexpected {} at top-level!", tree.kind),
                _ => {
                    functions.push(self.parse_function(&mut cursor));
                }
            }
        }
        Tree {
            functions,
            any_errors: false,
        }
    }
}

pub struct Cursor<'a> {
    trees: Vec<&'a TokenTree>,
    index: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(trees: &'a Vec<TokenTree>) -> Self {
        let trees = trees
            .iter()
            .filter(|tree| !tree.is_comment_or_whitespace())
            .collect();
        Self { trees, index: 0 }
    }

    pub fn peek(&self) -> Option<&'a TokenTree> {
        self.trees.get(self.index).map(|tree| *tree)
    }

    pub fn advance(&mut self) {
        self.index += 1;
    }
}

impl<'a> Cursor<'a> {
    pub fn expect_token(&mut self, kind: TokenKind) -> &'a Token {
        self.maybe_token(kind).unwrap()
    }

    pub fn maybe_token(&mut self, kind: TokenKind) -> Option<&'a Token> {
        let token = self.peek_token()?;
        if token.kind != kind {
            return None;
        }
        self.advance();
        Some(token)
    }

    pub fn peek_token(&self) -> Option<&'a Token> {
        self.peek().and_then(TokenTree::token)
    }
}

impl<'a> Cursor<'a> {
    pub fn expect_tree(&mut self, kind: TreeKind) -> &'a SubTree {
        self.maybe_tree(kind).unwrap()
    }

    pub fn maybe_tree(&mut self, kind: TreeKind) -> Option<&'a SubTree> {
        let tree = self.peek_tree()?;
        if tree.kind != kind {
            return None;
        }
        self.advance();
        Some(tree)
    }

    pub fn peek_tree(&mut self) -> Option<&'a SubTree> {
        self.peek().and_then(TokenTree::tree)
    }
}

impl<'a> Iterator for Cursor<'a> {
    type Item = &'a TokenTree;
    fn next(&mut self) -> Option<Self::Item> {
        let tree = self.peek()?;
        self.advance();
        Some(tree)
    }
}

impl itertools::PeekingNext for Cursor<'_> {
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
    where
        F: FnOnce(&Self::Item) -> bool,
    {
        let next = self.peek()?;
        if accept(&next) {
            self.next();
            return Some(next);
        }
        return None;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::{Parameter, Scope, Statement, Type, TypeKind, Variable};

    fn parse(source: &'static str) -> Tree {
        super::parse(crate::lexer::test::lex(source))
    }

    fn tree(functions: &[Function]) -> Tree {
        Tree {
            functions: functions.to_vec(),
            any_errors: false,
        }
    }

    fn function(
        name: &str,
        parameters: &[Parameter],
        return_type: Type,
        statements: Option<&[Statement]>,
    ) -> Function {
        let body = statements.map(|s| Scope {
            statements: s.to_vec(),
        });
        Function {
            identifier: name.to_string(),
            parameters: parameters.to_vec(),
            body,
            return_type,
        }
    }

    fn param(name: &str, type_: &str) -> Parameter {
        let type_ = ty(type_);
        let variable = var(name);
        Parameter { type_, variable }
    }

    fn ty(ty: &str) -> Type {
        use std::convert::TryFrom;
        let kind = TypeKind::try_from(ty).unwrap();
        Type { kind }
    }

    fn var(name: &str) -> Variable {
        let identifier = name.to_string();
        Variable { identifier }
    }

    #[test]
    fn empty() {
        assert_eq!(parse(""), tree(&[]))
    }

    #[test]
    fn empty_function() {
        assert_eq!(
            parse(
                "fn a(foo: i64, bar: i64) -> i32 {}\n\
                 fn a() -> i32 {}"
            ),
            tree(&[
                function(
                    "a",
                    &[param("foo", "i64"), param("bar", "i64")],
                    ty("i32"),
                    Some(&[]),
                ),
                function("a", &[], ty("i32"), Some(&[]))
            ])
        );
    }

    #[test]
    fn function_declaration() {
        assert_eq!(
            parse("fn a(foo: i64, bar: i64) -> i8;"),
            tree(&[function(
                "a",
                &[param("foo", "i64"), param("bar", "i64")],
                ty("i8"),
                None,
            )])
        );
    }

    #[test]
    #[should_panic(expected = "Unexpected braces at top-level!")]
    fn top_level_block() {
        parse("{}");
    }
}
