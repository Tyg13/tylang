use crate::CompletedMarker;

use super::Parser;
use cst::SyntaxKind::{self, *};
use cst::T;

mod expressions;
mod items;

pub enum EntryPoint {
    Module,
    Block,
    Expression,
}

pub(super) fn entry_point(parser: &mut Parser<'_>, entry: EntryPoint) {
    match entry {
        EntryPoint::Module => {
            module(parser, false);
        }
        EntryPoint::Block => {
            expressions::block(parser);
        }
        EntryPoint::Expression => {
            expressions::expr(parser);
        }
    }
}

fn module(parser: &mut Parser<'_>, inner_module: bool) {
    let m = parser.start_node();

    parser.advance_to_next_non_trivia();

    if inner_module {
        parser.expect_token(T![mod]);
        parser.expect_token(IDENT);
        parser.expect_token(T!['{']);
    }
    loop {
        match parser.advance_to_next_non_trivia() {
            T![mod] => module(parser, true),
            T![import] => items::import_item(parser),
            T![let] => items::let_item(parser),
            T![fn] => items::fn_item(parser),
            T![type] => items::type_item(parser),
            T!['}'] => {
                if inner_module {
                    break;
                } else {
                    parser.unexpected(T!['}'])
                }
            }
            EOF => {
                if inner_module {
                    parser.unexpected(EOF);
                }
                break;
            }
            _ => items::expr_item(parser),
        }
    }
    if inner_module {
        parser.expect_token(T!['}']);
    }

    m.complete(parser, MODULE);
}

fn name(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.advance_to_next_non_trivia();
    let m = parser.start_node();
    parser.expect_token(IDENT);
    if parser.maybe(T![::]) {
        parser.expect_token(T![::]);
        name(parser);
        m.complete(parser, DOTTED_NAME)
    } else {
        m.complete(parser, NAME)
    }
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
                name(parser);
            });
        }
    }
}

fn param_list(parser: &mut Parser<'_>) {
    parser.node(PARAM_LIST, |parser| {
        parser.expect_token(T!['(']);
        parser.with_follow_set(&[T![')']], |parser| loop {
            parser.add_to_follow_set(&[T![:], T![,]]);
            match parser.advance_to_next_non_trivia() {
                T![')'] | EOF => break,
                T![.] => {
                    parser.node(VA_PARAM, |parser| {
                        parser.expect_token(T![...]);
                    });
                    break;
                }
                _ => {
                    param(parser);
                    if parser.maybe(T![,]) {
                        parser.token(T![,]);
                    }
                }
            }
        });
        parser.expect_token(T![')']);
    });
}

fn param(parser: &mut Parser<'_>) {
    parser.node(PARAM, |parser| {
        name(parser);
        parser.expect_token(T![:]);
        type_(parser);
    });
}
