use crate::grammar::*;
use cst::T;

pub(super) fn let_item(parser: &mut Parser<'_>) {
    parser.node(LET_ITEM, |parser| {
        parser.with_follow_set(&[T![:], T![=], T![;]], |parser| {
            parser.expect_token(T![let]);
            name(parser);
            if parser.maybe(T![:]) {
                parser.expect_token(T![:]);
                type_(parser);
            }
            if parser.maybe(T![=]) {
                parser.expect_token(T![=]);
                if expressions::expr(parser).is_none() {
                    parser.error("No expression");
                }
            }
            parser.expect_token(T![;]);
        });
    });
}

pub(super) fn fn_item(parser: &mut Parser<'_>) {
    parser.node(FN_ITEM, |parser| {
        parser.expect_token(T![fn]);
        parser.with_follow_set(&[T!['(']], |parser| {
            name(parser);
        });
        param_list(parser);
        if parser.maybe(T![->]) {
            parser.expect_token(T![->]);
            type_(parser);
        }
        parser.expect_token(T![=]);
        if expressions::expr(parser).is_some() {
            return;
        } else {
            parser.maybe_token(T![extern]);
            parser.expect_token(T![;]);
        }
    });
}

pub(super) fn expr_item(parser: &mut Parser<'_>) {
    parser.node(EXPR_ITEM, |parser| {
        parser.with_follow_set(&[T![;]], |parser| {
            let expr = expressions::expr(parser);
            let expect_semi =
                expr.map_or(true, |e| e.kind().terminated_by_semicolon());
            if expect_semi {
                parser.expect_token(T![;]);
            }
        })
    });
}

macro_rules! until_unexpected_match_next_non_trivia {
    ($parser:expr, $($p:pat => $body:tt$(,)?)*) => {
        loop {
            match $parser.advance_to_next_non_trivia() {
                $($p => $body),*,
                kind => {
                    $parser.unexpected(kind);
                    break;
                }
            }
        }
    }
}

pub(super) fn type_item(parser: &mut Parser<'_>) {
    parser.node(TYPE_ITEM, |parser| {
        parser.expect_token(T![type]);
        parser.expect_token(IDENT);
        parser.expect_token(T!['{']);
        parser.add_to_follow_set(&[T!['}']]);
        until_unexpected_match_next_non_trivia! {parser,
            IDENT => {
                parser.add_to_follow_set(&[T![,]]);
                type_member(parser);
                if parser.maybe(T![,]) {
                    parser.token(T![,]);
                }
            }
            T!['}'] => break,
        }
        parser.expect_token(T!['}']);
    });
}

fn type_member(parser: &mut Parser) {
    parser.node(TYPE_MEMBER, |parser| {
        parser.expect_token(IDENT);
        parser.expect_token(T![:]);
        type_(parser);
    });
}

pub(super) fn import_item(parser: &mut Parser) {
    parser.node(IMPORT_ITEM, |parser| {
        parser.expect_token(T![import]);
        expressions::name_ref(parser);
        parser.expect_token(T![;]);
    });
}

#[cfg(test)]
mod tests {
    use crate::tests::check_tree;

    #[test]
    fn let_no_expr() {
        check_tree(
            "let foo : bar;",
            expect_test::expect![[r#"
                MODULE @ 0..14:
                  LET_ITEM @ 0..14:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..7:
                      IDENT @ 4..7: 'foo' 
                    WHITESPACE @ 7..8: ' ' 
                    COLON @ 8..9: ':' 
                    WHITESPACE @ 9..10: ' ' 
                    BASIC_TYPE @ 10..13:
                      NAME @ 10..13:
                        IDENT @ 10..13: 'bar' 
                    SEMICOLON @ 13..14: ';' "#]],
        );
    }

    #[test]
    fn let_with_expr() {
        check_tree(
            "let foo : bar = 10;",
            expect_test::expect![[r#"
                MODULE @ 0..19:
                  LET_ITEM @ 0..19:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..7:
                      IDENT @ 4..7: 'foo' 
                    WHITESPACE @ 7..8: ' ' 
                    COLON @ 8..9: ':' 
                    WHITESPACE @ 9..10: ' ' 
                    BASIC_TYPE @ 10..13:
                      NAME @ 10..13:
                        IDENT @ 10..13: 'bar' 
                    WHITESPACE @ 13..14: ' ' 
                    EQUALS @ 14..15: '=' 
                    WHITESPACE @ 15..16: ' ' 
                    LITERAL @ 16..18:
                      NUMBER @ 16..18: '10' 
                    SEMICOLON @ 18..19: ';' "#]],
        );
    }

    #[test]
    fn type_() {
        check_tree(
            "type A { first: i32, second: i32 }",
            expect_test::expect![[r#"
                MODULE @ 0..34:
                  TYPE_ITEM @ 0..34:
                    TYPE_KW @ 0..4: 'type' 
                    WHITESPACE @ 4..5: ' ' 
                    IDENT @ 5..6: 'A' 
                    WHITESPACE @ 6..7: ' ' 
                    LEFT_CURLY @ 7..8: '{' 
                    WHITESPACE @ 8..9: ' ' 
                    TYPE_MEMBER @ 9..19:
                      IDENT @ 9..14: 'first' 
                      COLON @ 14..15: ':' 
                      WHITESPACE @ 15..16: ' ' 
                      BASIC_TYPE @ 16..19:
                        NAME @ 16..19:
                          IDENT @ 16..19: 'i32' 
                    COMMA @ 19..20: ',' 
                    WHITESPACE @ 20..21: ' ' 
                    TYPE_MEMBER @ 21..32:
                      IDENT @ 21..27: 'second' 
                      COLON @ 27..28: ':' 
                      WHITESPACE @ 28..29: ' ' 
                      BASIC_TYPE @ 29..32:
                        NAME @ 29..32:
                          IDENT @ 29..32: 'i32' 
                    WHITESPACE @ 32..33: ' ' 
                    RIGHT_CURLY @ 33..34: '}' "#]],
        );
    }
}
