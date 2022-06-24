use super::Parser;
use crate::SyntaxKind::{self, *};
use crate::T;

mod expressions;
mod items;

pub enum EntryPoint {
    Module,
    Expression,
}

pub(super) fn entry_point(parser: &mut Parser<'_>, entry: EntryPoint) {
    match entry {
        EntryPoint::Module => {
            module(parser);
        }
        EntryPoint::Expression => {
            expressions::expr(parser);
        }
    }
}

fn module(parser: &mut Parser<'_>) {
    parser.node(MODULE, |parser| {
        loop {
            match parser.advance_to_next_non_trivia() {
                T![let] => items::let_item(parser),
                T![fn] => items::fn_item(parser),
                T![type] => items::type_item(parser),
                EOF => break,
                _ => items::expr_item(parser),
            }
        }
    });
}

// TODO: are there usages of name that are unnecessary (why can't we just use IDENT?)
fn name(parser: &mut Parser<'_>) {
    parser.node(NAME, |parser| {
        parser.expect_token(IDENT);
    });
}

fn type_(parser: &mut Parser<'_>) {
    match parser.advance_to_next_non_trivia() {
        T![*] => {
            parser.node(POINTER_TYPE, |parser| {
                parser.expect_token(T![*]);
                type_(parser);
            });
        }
        _ => {
            parser.node(BASIC_TYPE, |parser| {
                parser.expect_token(IDENT);
            });
        }
    }
}

fn param_list(parser: &mut Parser<'_>) {
    parser.node(PARAM_LIST, |this| {
        this.with_follow_set(&[T!['(']], |parser| {
            parser.expect_token(T!['(']);
            loop {
                parser.add_to_follow_set(&[T![:], T![,]]);
                match parser.advance_to_next_non_trivia() {
                    T![')'] | EOF => break,
                    T![.] => {
                        parser.node(VA_PARAM, |parser| parser.expect_token(T![...]));
                        break;
                    }
                    _ => {
                        param(parser);
                        if parser.maybe(T![,]) {
                            parser.token(T![,]);
                        }
                    }
                }
            }
            parser.expect_token(T![')']);
        });
    });
}

fn param(parser: &mut Parser<'_>) {
    parser.node(PARAM, |parser| {
        name(parser);
        parser.expect_token(T![:]);
        type_(parser);
    });
}
