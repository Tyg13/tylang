use crate::CompletedMarker;
use cst::T;

use super::*;

pub(super) fn expr(parser: &mut Parser<'_>) -> Option<CompletedMarker> {
    expr_with_precedence(parser, 0)
}

fn expr_with_precedence(
    parser: &mut Parser<'_>,
    min_precedence: usize,
) -> Option<CompletedMarker> {
    // Parse LHS (including any prefix ops)
    let mut lhs = expr_lhs(parser)?;
    loop {
        // Parse optional postfix op
        lhs = postfix_op(parser, lhs, min_precedence);

        // Parse optional infix op
        match infix_op(parser) {
            Some((op, left_binding, right_binding)) => {
                // Check the binding power -- if we bind greater on the left than the
                // previous op binds on the right, then we should create a BIN_EXPR
                // here, e.g:
                //
                // 1 + 2 * 3
                //  1 2 2 3
                if left_binding < min_precedence {
                    break;
                }
                lhs = match op {
                    T![as] => as_expr(parser, lhs),
                    op => {
                        let node = lhs.precede(parser);
                        parser.token(op);
                        expr_with_precedence(parser, right_binding);
                        node.complete(parser, BIN_EXPR)
                    }
                };
            }
            _ => break,
        }
    }
    Some(lhs)
}

fn expr_lhs(parser: &mut Parser) -> Option<CompletedMarker> {
    Some(match parser.advance_to_next_non_trivia() {
        NUMBER | STRING => literal(parser),
        IDENT => {
            let n = parser.start_node();
            name(parser);
            if parser.maybe(T!['{']) {
                parser.expect_token(T!['{']);
                parser.expect_token(T!['}']);
                n.complete(parser, STRUCT_LITERAL)
            } else {
                n.complete(parser, NAME_REF)
            }
        }
        T![if] => if_expr(parser),
        T![loop] => loop_expr(parser),
        T![while] => while_expr(parser),
        T![break] => break_expr(parser),
        T![continue] => continue_expr(parser),
        T!['('] => paren(parser),
        T!['{'] => block(parser),
        T![return] => return_(parser),
        op => match prefix_binding_power(op) {
            Some(((), right_binding)) => {
                let node = parser.start_node();
                parser.token(op);
                expr_with_precedence(parser, right_binding);
                node.complete(parser, PREFIX_EXPR)
            }
            _ => return None,
        },
    })
}

fn paren(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(PAREN_EXPR, |parser| {
        parser.expect_token(T!['(']);
        parser.with_follow_set(&[T![')']], |parser| {
            expr(parser);
            parser.expect_token(T![')']);
        });
    })
}

pub(crate) fn name_ref(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(NAME_REF, |parser| {
        match parser.advance_to_next_non_trivia() {
            IDENT => {
                name(parser);
            }
            kind => parser.unexpected(kind),
        }
    })
}

fn if_expr(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(IF_EXPR, |parser| {
        parser.expect_token(T![if]);
        expr(parser);
        block(parser);
        if parser.maybe(T![else]) {
            parser.expect_token(T![else]);
            block(parser);
        }
    })
}

fn loop_expr(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(LOOP_EXPR, |parser| {
        parser.expect_token(T![loop]);
        block(parser);
    })
}

fn while_expr(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(WHILE_EXPR, |parser| {
        parser.expect_token(T![while]);
        expr(parser);
        block(parser);
    })
}

fn break_expr(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(BREAK_EXPR, |parser| {
        parser.expect_token(T![break]);
        expr(parser);
    })
}

fn continue_expr(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(CONTINUE_EXPR, |parser| {
        parser.expect_token(T![continue]);
    })
}

pub(super) fn block(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(BLOCK_EXPR, |parser| {
        parser.expect_token(T!['{']);
        block_inner(parser);
        parser.expect_token(T!['}']);
    })
}

pub(super) fn block_inner(parser: &mut Parser<'_>) {
    loop {
        parser.step();
        match parser.advance_to_next_non_trivia() {
            T![let] => {
                items::let_item(parser);
            }
            T![fn] => {
                items::fn_item(parser);
            }
            EOF => {
                parser.unexpected(EOF);
                break;
            }
            T!['}'] => break,
            _ => {
                if let Some(expr) = expr(parser) {
                    // Anything followed by a semicolon is an expr item,
                    // even if it doesn't need to be terminated by a semicolon
                    // (e.g. if expressions).
                    if parser.maybe(T![;]) {
                        let item = expr.precede(parser);
                        parser.expect_token(T![;]);
                        item.complete(parser, EXPR_ITEM);
                        continue;
                    }

                    // Otherwise, we know there's no terminating semicolon.
                    // If this expr must be terminated by a semicolon, then
                    // it must be the last expression in the block.
                    if expr.kind().terminated_by_semicolon() {
                        break;
                    }

                    // Otherwise, this is an expression with no terminating
                    // semicolon (and no semicolon at the current position).
                    // Break if this is the last expr in the block.
                    if parser.maybe(T!['}']) {
                        break;
                    }

                    // Finally, we must have an expression with no terminating
                    // semicolon, no semicolon at the current position, and
                    // this *isn't* the last expr in the block (i.e. this is an item).
                    // Convert the expr into an item and continue.
                    expr.precede(parser).complete(parser, EXPR_ITEM);
                }
            }
        }
    }
}

fn return_(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(RETURN_EXPR, |parser| {
        parser.expect_token(T![return]);
        expr(parser);
    })
}

fn literal(parser: &mut Parser<'_>) -> CompletedMarker {
    parser.node(LITERAL, |parser| {
        match parser.advance_to_next_non_trivia() {
            NUMBER => parser.token(NUMBER),
            STRING => parser.token(STRING),
            kind => parser.unexpected(kind),
        }
    })
}

fn infix_op(parser: &mut Parser<'_>) -> Option<(SyntaxKind, usize, usize)> {
    let op_kind = match parser.advance_to_next_non_trivia() {
        T![|] if parser.at(T![||]) => T![||],
        T![&] if parser.at(T![&&]) => T![&&],
        T![=] if parser.at(T![==]) => T![==],
        T![!] if parser.at(T![!=]) => T![!=],
        T![<] if parser.at(T![<=]) => T![<=],
        T![>] if parser.at(T![>=]) => T![>=],
        T![-] if parser.at(T![->]) => T![->],
        kind => kind,
    };
    let (left_binding, right_binding) = infix_binding_power(op_kind)?;
    Some((op_kind, left_binding, right_binding))
}

#[rustfmt::skip]
fn infix_binding_power(kind: SyntaxKind) -> Option<(usize, usize)> {
    match kind {
        T![=]                    => Some((0, 1)),
        T![&&] | T![||]          => Some((1, 2)),
        T![==] | T![!=] | T![<=] | T![>=] 
               | T![<]  | T![>]  => Some((2, 3)),
        T![+]  | T![-]           => Some((3, 4)),
        T![*]  | T![/]           => Some((4, 5)),
        T![.]  | T![->] | T![as] => Some((5, 6)),
        _ => None,
    }
}

fn prefix_binding_power(kind: SyntaxKind) -> Option<((), usize)> {
    match kind {
        T![-] | T![+] | T![*] => Some(((), 5)),
        _ => None,
    }
}

fn postfix_binding_power(kind: SyntaxKind) -> Option<(usize, ())> {
    match kind {
        T!['('] | T!['['] => Some((5, ())),
        _ => None,
    }
}

fn postfix_op(
    parser: &mut Parser<'_>,
    lhs: CompletedMarker,
    min_precedence: usize,
) -> CompletedMarker {
    let op_kind = parser.advance_to_next_non_trivia();
    if let Some((left_binding, ())) = postfix_binding_power(op_kind) {
        if left_binding < min_precedence {
            return lhs;
        }
        match op_kind {
            T!['('] => return call_expr(parser, lhs),
            T!['['] => return index_expr(parser, lhs),
            _ => {}
        }
    }
    lhs
}

fn call_expr(parser: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    let node = lhs.precede(parser);
    parser.expect_token(T!['(']);
    loop {
        match parser.advance_to_next_non_trivia() {
            T![')'] => {
                parser.expect_token(T![')']);
                break;
            }
            EOF => {
                parser.unexpected(EOF);
                break;
            }
            _ => {
                expr(parser);
                if parser.maybe(T![,]) {
                    parser.expect_token(T![,]);
                }
            }
        }
    }
    node.complete(parser, CALL_EXPR)
}

fn index_expr(
    parser: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> CompletedMarker {
    let node = lhs.precede(parser);
    parser.expect_token(T!['[']);
    expr(parser);
    parser.expect_token(T![']']);
    node.complete(parser, INDEX_EXPR)
}

fn as_expr(parser: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    let node = lhs.precede(parser);
    parser.expect_token(T![as]);
    type_(parser);
    node.complete(parser, AS_EXPR)
}

#[cfg(test)]
mod tests {
    use crate::tests::check_tree;
    use expect_test::expect;

    #[test]
    fn bin_expr_no_precedence() {
        check_tree(
            "let i: i32 = a+b;",
            expect![[r#"
                MODULE @ 0..17:
                  LET_ITEM @ 0..17:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    BIN_EXPR @ 13..16:
                      NAME_REF @ 13..14:
                        NAME @ 13..14:
                          IDENT @ 13..14: 'a' 
                      PLUS @ 14..15: '+' 
                      NAME_REF @ 15..16:
                        NAME @ 15..16:
                          IDENT @ 15..16: 'b' 
                    SEMICOLON @ 16..17: ';' "#]],
        );
    }

    #[test]
    fn bin_expr_left_precedence() {
        check_tree(
            "let i: i32 = a+b+c;",
            expect![[r#"
                MODULE @ 0..19:
                  LET_ITEM @ 0..19:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    BIN_EXPR @ 13..18:
                      BIN_EXPR @ 13..16:
                        NAME_REF @ 13..14:
                          NAME @ 13..14:
                            IDENT @ 13..14: 'a' 
                        PLUS @ 14..15: '+' 
                        NAME_REF @ 15..16:
                          NAME @ 15..16:
                            IDENT @ 15..16: 'b' 
                      PLUS @ 16..17: '+' 
                      NAME_REF @ 17..18:
                        NAME @ 17..18:
                          IDENT @ 17..18: 'c' 
                    SEMICOLON @ 18..19: ';' "#]],
        );
    }

    #[test]
    fn bin_expr_right_precedence() {
        check_tree(
            "let i: i32 = a+b*c;",
            expect![[r#"
                MODULE @ 0..19:
                  LET_ITEM @ 0..19:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    BIN_EXPR @ 13..18:
                      NAME_REF @ 13..14:
                        NAME @ 13..14:
                          IDENT @ 13..14: 'a' 
                      PLUS @ 14..15: '+' 
                      BIN_EXPR @ 15..18:
                        NAME_REF @ 15..16:
                          NAME @ 15..16:
                            IDENT @ 15..16: 'b' 
                        STAR @ 16..17: '*' 
                        NAME_REF @ 17..18:
                          NAME @ 17..18:
                            IDENT @ 17..18: 'c' 
                    SEMICOLON @ 18..19: ';' "#]],
        );
    }

    #[test]
    fn paren_expr() {
        check_tree(
            "let i: i32 = ((a));",
            expect![[r#"
                MODULE @ 0..19:
                  LET_ITEM @ 0..19:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    PAREN_EXPR @ 13..18:
                      LEFT_PAREN @ 13..14: '(' 
                      PAREN_EXPR @ 14..17:
                        LEFT_PAREN @ 14..15: '(' 
                        NAME_REF @ 15..16:
                          NAME @ 15..16:
                            IDENT @ 15..16: 'a' 
                        RIGHT_PAREN @ 16..17: ')' 
                      RIGHT_PAREN @ 17..18: ')' 
                    SEMICOLON @ 18..19: ';' "#]],
        );
    }

    #[test]
    fn bin_expr_with_paren() {
        check_tree(
            "let i: i32 = (a+b)*c;",
            expect![[r#"
                MODULE @ 0..21:
                  LET_ITEM @ 0..21:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    BIN_EXPR @ 13..20:
                      PAREN_EXPR @ 13..18:
                        LEFT_PAREN @ 13..14: '(' 
                        BIN_EXPR @ 14..17:
                          NAME_REF @ 14..15:
                            NAME @ 14..15:
                              IDENT @ 14..15: 'a' 
                          PLUS @ 15..16: '+' 
                          NAME_REF @ 16..17:
                            NAME @ 16..17:
                              IDENT @ 16..17: 'b' 
                        RIGHT_PAREN @ 17..18: ')' 
                      STAR @ 18..19: '*' 
                      NAME_REF @ 19..20:
                        NAME @ 19..20:
                          IDENT @ 19..20: 'c' 
                    SEMICOLON @ 20..21: ';' "#]],
        );
    }

    #[test]
    fn prefix_expr() {
        check_tree(
            "let i: i32 = --c;",
            expect![[r#"
                MODULE @ 0..17:
                  LET_ITEM @ 0..17:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    PREFIX_EXPR @ 13..16:
                      DASH @ 13..14: '-' 
                      PREFIX_EXPR @ 14..16:
                        DASH @ 14..15: '-' 
                        NAME_REF @ 15..16:
                          NAME @ 15..16:
                            IDENT @ 15..16: 'c' 
                    SEMICOLON @ 16..17: ';' "#]],
        );
    }

    #[test]
    fn prefix_expr_with_paren_bin_expr() {
        check_tree(
            "let i: i32 = -(c * 2);",
            expect![[r#"
                MODULE @ 0..22:
                  LET_ITEM @ 0..22:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    PREFIX_EXPR @ 13..21:
                      DASH @ 13..14: '-' 
                      PAREN_EXPR @ 14..21:
                        LEFT_PAREN @ 14..15: '(' 
                        BIN_EXPR @ 15..20:
                          NAME_REF @ 15..16:
                            NAME @ 15..16:
                              IDENT @ 15..16: 'c' 
                          WHITESPACE @ 16..17: ' ' 
                          STAR @ 17..18: '*' 
                          WHITESPACE @ 18..19: ' ' 
                          LITERAL @ 19..20:
                            NUMBER @ 19..20: '2' 
                        RIGHT_PAREN @ 20..21: ')' 
                    SEMICOLON @ 21..22: ';' "#]],
        );
    }

    #[test]
    fn bin_expr_with_prefix_expr() {
        check_tree(
            "let i: i32 = -c * 2;",
            expect![[r#"
                MODULE @ 0..20:
                  LET_ITEM @ 0..20:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    COLON @ 5..6: ':' 
                    WHITESPACE @ 6..7: ' ' 
                    BASIC_TYPE @ 7..10:
                      NAME @ 7..10:
                        IDENT @ 7..10: 'i32' 
                    WHITESPACE @ 10..11: ' ' 
                    EQUALS @ 11..12: '=' 
                    WHITESPACE @ 12..13: ' ' 
                    BIN_EXPR @ 13..19:
                      PREFIX_EXPR @ 13..16:
                        DASH @ 13..14: '-' 
                        NAME_REF @ 14..15:
                          NAME @ 14..15:
                            IDENT @ 14..15: 'c' 
                        WHITESPACE @ 15..16: ' ' 
                      STAR @ 16..17: '*' 
                      WHITESPACE @ 17..18: ' ' 
                      LITERAL @ 18..19:
                        NUMBER @ 18..19: '2' 
                    SEMICOLON @ 19..20: ';' "#]],
        );
    }

    #[test]
    #[ignore]
    fn block_expr() {
        check_tree(
            "let i = {10};",
            expect![[r#"
MODULE:
  LET_ITEM:
    LET_KW: 'let'
    WHITESPACE: ' '
    NAME:
      IDENT: 'i'
    WHITESPACE: ' '
    EQUALS: '='
    WHITESPACE: ' '
    BLOCK_EXPR:
      LEFT_CURLY: '{'
      RIGHT_CURLY: '}'
        "#]],
        );
    }

    #[test]
    fn bin_op_with_equals_equals() {
        check_tree(
            "let i = a == 10 || b == 20;",
            expect![[r#"
                MODULE @ 0..27:
                  LET_ITEM @ 0..27:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    WHITESPACE @ 5..6: ' ' 
                    EQUALS @ 6..7: '=' 
                    WHITESPACE @ 7..8: ' ' 
                    BIN_EXPR @ 8..26:
                      BIN_EXPR @ 8..16:
                        NAME_REF @ 8..9:
                          NAME @ 8..9:
                            IDENT @ 8..9: 'a' 
                        WHITESPACE @ 9..10: ' ' 
                        EQUALS_EQUALS @ 10..12: '==' 
                        WHITESPACE @ 12..13: ' ' 
                        LITERAL @ 13..15:
                          NUMBER @ 13..15: '10' 
                        WHITESPACE @ 15..16: ' ' 
                      BAR_BAR @ 16..18: '||' 
                      WHITESPACE @ 18..19: ' ' 
                      BIN_EXPR @ 19..26:
                        NAME_REF @ 19..20:
                          NAME @ 19..20:
                            IDENT @ 19..20: 'b' 
                        WHITESPACE @ 20..21: ' ' 
                        EQUALS_EQUALS @ 21..23: '==' 
                        WHITESPACE @ 23..24: ' ' 
                        LITERAL @ 24..26:
                          NUMBER @ 24..26: '20' 
                    SEMICOLON @ 26..27: ';' "#]],
        );
    }

    #[test]
    fn postfix_op_call_expr() {
        check_tree(
            "let i = foo();",
            expect![[r#"
                MODULE @ 0..14:
                  LET_ITEM @ 0..14:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: 'i' 
                    WHITESPACE @ 5..6: ' ' 
                    EQUALS @ 6..7: '=' 
                    WHITESPACE @ 7..8: ' ' 
                    CALL_EXPR @ 8..13:
                      NAME_REF @ 8..11:
                        NAME @ 8..11:
                          IDENT @ 8..11: 'foo' 
                      LEFT_PAREN @ 11..12: '(' 
                      RIGHT_PAREN @ 12..13: ')' 
                    SEMICOLON @ 13..14: ';' "#]],
        );
    }

    #[test]
    fn mixed_ops() {
        check_tree(
            "let _ = -foo.bar() + -baz();",
            expect![[r#"
                MODULE @ 0..28:
                  LET_ITEM @ 0..28:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: '_' 
                    WHITESPACE @ 5..6: ' ' 
                    EQUALS @ 6..7: '=' 
                    WHITESPACE @ 7..8: ' ' 
                    BIN_EXPR @ 8..27:
                      PREFIX_EXPR @ 8..19:
                        DASH @ 8..9: '-' 
                        CALL_EXPR @ 9..18:
                          BIN_EXPR @ 9..16:
                            NAME_REF @ 9..12:
                              NAME @ 9..12:
                                IDENT @ 9..12: 'foo' 
                            DOT @ 12..13: '.' 
                            NAME_REF @ 13..16:
                              NAME @ 13..16:
                                IDENT @ 13..16: 'bar' 
                          LEFT_PAREN @ 16..17: '(' 
                          RIGHT_PAREN @ 17..18: ')' 
                        WHITESPACE @ 18..19: ' ' 
                      PLUS @ 19..20: '+' 
                      WHITESPACE @ 20..21: ' ' 
                      PREFIX_EXPR @ 21..27:
                        DASH @ 21..22: '-' 
                        CALL_EXPR @ 22..27:
                          NAME_REF @ 22..25:
                            NAME @ 22..25:
                              IDENT @ 22..25: 'baz' 
                          LEFT_PAREN @ 25..26: '(' 
                          RIGHT_PAREN @ 26..27: ')' 
                    SEMICOLON @ 27..28: ';' "#]],
        );
    }
    #[test]
    fn as_() {
        check_tree(
            "let _ = 10 as i32;",
            expect![[r#"
                MODULE @ 0..18:
                  LET_ITEM @ 0..18:
                    LET_KW @ 0..3: 'let' 
                    WHITESPACE @ 3..4: ' ' 
                    NAME @ 4..5:
                      IDENT @ 4..5: '_' 
                    WHITESPACE @ 5..6: ' ' 
                    EQUALS @ 6..7: '=' 
                    WHITESPACE @ 7..8: ' ' 
                    AS_EXPR @ 8..17:
                      LITERAL @ 8..10:
                        NUMBER @ 8..10: '10' 
                      WHITESPACE @ 10..11: ' ' 
                      AS_KW @ 11..13: 'as' 
                      WHITESPACE @ 13..14: ' ' 
                      BASIC_TYPE @ 14..17:
                        NAME @ 14..17:
                          IDENT @ 14..17: 'i32' 
                    SEMICOLON @ 17..18: ';' "#]],
        )
    }
}
