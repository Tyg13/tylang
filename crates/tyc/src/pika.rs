use std::convert::TryFrom;

use ast::Tree;
use lexer::{TokenKind, Tokens};

struct Parser<'tokens> {
    tokens: &'tokens Tokens<'tokens>,
    table: MemoTable,
    seed_clauses: SeedClauses,
}

type MemoKey = (ClauseKind, usize);
type MemoTable = std::collections::HashMap<MemoKey, Match>;
type SeedClauses = std::collections::BinaryHeap<MemoKey>;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Match {
    len: usize,
}
//
// Ast rough sketch (future, not as currently implemented)
//
// Tree ::== [Item]*
// Item ::= Function
// Function ::= "fn" Ident "(" [Param ","]* Param? ")" ["->" Type]? [Scope / ";"]
// Param ::= Variable ":" Type
// Variable ::= Ident
// Expr ::= Scope / Group / Binary / Return
// Scope ::= "{" [Statement ";"]* [Expr]? "}"
// Statement ::= Decl / Expr
// Decl ::= "let" Variable [":" Type]? ["=" Expr]?
// Group ::= "(" Expr ")"
// Binary ::= Expr BinOp Expr
// Return ::= "return" Expr
//
// Tree -> Item -> Function -> "fn"
//                          -> Ident
//                          -> "("
//                          -> Param -> Variable
//                                   -> ":"
//                                   -> Type
//                          -> ","
//                          -> ")"
//                          -> "->"
//                          -> ";"
//                          -> Scope -> "{"
//                                   -> Statement -> Decl -> "let"
//                                                        -> "="
//                                                        -> Expr -> Group -> Binary -> Return
//                                   -> "}"

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ClauseKind {
    Tree,
    Item,
    Function,
    Param,
    Variable,
    Type,
    Scope,
    Statement,
    Decl,
    Expr,
    Group,
    Return,
    Ident,
    Fn,
    LParen,
    Colon,
    Comma,
    RParen,
    Arrow,
    SemiColon,
    LBrace,
    Let,
    Equals,
    RBrace,
}

impl<'tokens> Parser<'tokens> {
    fn new(tokens: &'tokens Tokens<'tokens>) -> Self {
        Self {
            tokens,
            table: MemoTable::new(),
            seed_clauses: SeedClauses::new(),
        }
    }

    fn parse(&mut self) -> Option<()> {
        let start_key = (ClauseKind::Tree, 0);

        self.parse_terminals();
        if self.tokens.len() == 0 {
            self.seed_clauses.push(start_key);
        }
        self.parse_non_terminals();

        &self.table.get(&start_key).map(|clause| ())
    }

    fn parse_terminals(&mut self) {
        self.tokens
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(index, token)| {
                let mut seed_clauses = Vec::new();
                let kind = match token.kind() {
                    TokenKind::Fn => {
                        seed_clauses.push((ClauseKind::Function, index));
                        ClauseKind::Fn
                    }
                    TokenKind::Identifier => {
                        seed_clauses.push((ClauseKind::Variable, index));
                        seed_clauses.push((ClauseKind::Type, index));
                        ClauseKind::Ident
                    }
                    TokenKind::LeftParen => {
                        //seed_clauses.push((ClauseKind::Group, index));
                        ClauseKind::LParen
                    }
                    TokenKind::RightParen => ClauseKind::RParen,
                    TokenKind::LeftBrace => {
                        seed_clauses.push((ClauseKind::Scope, index));
                        ClauseKind::LBrace
                    }
                    TokenKind::MinusArrow => ClauseKind::Arrow,
                    TokenKind::RightBrace => ClauseKind::RBrace,
                    TokenKind::Colon => ClauseKind::Colon,
                    TokenKind::Comma => ClauseKind::Comma,
                    TokenKind::SemiColon => ClauseKind::SemiColon,
                    TokenKind::Let => ClauseKind::Let,
                    TokenKind::Equals => ClauseKind::Equals,
                    _ => return None,
                };
                Some((index, kind, seed_clauses))
            })
            .for_each(|(index, kind, seed_clauses)| {
                self.table.insert((kind, index), Match { len: 1 });
                self.seed_clauses.extend(seed_clauses.into_iter());
            })
    }

    fn parse_non_terminals(&mut self) {
        while let Some((kind, pos)) = self.seed_clauses.pop() {
            use ClauseKind::*;
            match kind {
                Tree => self.parse_tree(pos),
                Item => self.parse_item(pos),
                Function => self.parse_function(pos),
                Param => self.parse_param(pos),
                Type => self.parse_type(pos),
                Variable => self.parse_variable(pos),
                _ => panic!("Something horrible has happened"),
            }
        }
    }

    fn matches(&self, kind: ClauseKind, pos: usize) -> Option<Match> {
        self.table.get(&(kind, pos)).copied()
    }

    fn parse_tree(&mut self, pos: usize) {
        let mut len = 0;
        while let Some(item) = self.table.get(&(ClauseKind::Item, pos + len)) {
            len += item.len;
        }
        // If all tokens have been consumed (aka there are no error regions)
        if pos + len >= self.tokens.len() {
            self.table.insert((ClauseKind::Tree, pos), Match { len });
        }
    }

    fn parse_item(&mut self, pos: usize) {
        self.matches(ClauseKind::Function, pos).map(|function| {
            self.table.insert((ClauseKind::Item, pos), function);
            self.seed_clauses.push((ClauseKind::Tree, pos));
        });
    }

    fn parse_function(&mut self, pos: usize) {
        if let Some(len) = (|| {
            let mut curr = pos;
            self.matches(ClauseKind::Fn, curr)?;
            curr += 1;
            self.matches(ClauseKind::Ident, curr)?;
            curr += 1;
            self.matches(ClauseKind::LParen, curr)?;
            curr += 1;
            while let Some(len) = self.matches(ClauseKind::Param, curr).and_then(|param| {
                self.matches(ClauseKind::Comma, curr + 1)
                    .map(|_| param.len + 1)
            }) {
                curr += len;
            }
            if let Some(param) = self.matches(ClauseKind::Param, curr) {
                curr += param.len;
            }
            self.matches(ClauseKind::RParen, curr)?;
            curr += 1;
            if self.matches(ClauseKind::Arrow, curr).is_some() {
                curr += 1;
                self.matches(ClauseKind::Type, curr)?;
                curr += 1;
            }
            if let Some(scope) = self.matches(ClauseKind::Scope, curr) {
                curr += scope.len;
            } else {
                self.matches(ClauseKind::SemiColon, curr)?;
                curr += 1;
            }
            Some(curr - pos)
        })() {
            self.table
                .insert((ClauseKind::Function, pos), Match { len });
            self.seed_clauses.push((ClauseKind::Item, pos));
        }
    }

    fn parse_variable(&mut self, pos: usize) {
        self.matches(ClauseKind::Ident, pos).map(|ident| {
            self.table.insert((ClauseKind::Variable, pos), ident);
            self.seed_clauses.push((ClauseKind::Param, pos));
        });
    }

    fn parse_param(&mut self, pos: usize) {
        if self
            .matches(ClauseKind::Variable, pos)
            .and_then(|_| self.matches(ClauseKind::Colon, pos + 1))
            .and_then(|_| self.matches(ClauseKind::Type, pos + 2))
            .is_some()
        {
            self.table
                .insert((ClauseKind::Param, pos), Match { len: 3 });
        }
    }

    fn parse_type(&mut self, pos: usize) {
        self.matches(ClauseKind::Ident, pos).map(|ident| {
            self.table.insert((ClauseKind::Type, pos), ident);
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse(s: &str) {
        let source = crate::util::SourceBuilder::new().source(s).build();
        let map = lexer::lex(&source);
        let tokens = map.tokens().strip();
        Parser::new(&tokens).parse().unwrap()
    }

    #[test]
    fn empty() {
        parse("");
    }

    #[test]
    fn function() {
        parse("fn a(b: c) -> d;");
    }
}
