#[allow(non_camel_case_types)]
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum SyntaxKind {
    TOMBSTONE,
    ERROR,
    EOF,

    MODULE,
    NAME,

    BASIC_TYPE,
    POINTER_TYPE,

    PARAM_LIST,
    PARAM,
    VA_PARAM,

    LET_ITEM,
    FN_ITEM,
    EXPR_ITEM,
    TYPE_ITEM,

    TYPE_MEMBER,

    IDENT,
    NUMBER,
    WHITESPACE,
    STRING,

    LEFT_PAREN,
    RIGHT_PAREN,
    LEFT_CURLY,
    RIGHT_CURLY,
    LEFT_SQUARE,
    RIGHT_SQUARE,
    LEFT_ANGLE,
    RIGHT_ANGLE,

    COLON,
    SEMICOLON,
    AMPERSAND,
    EQUALS,
    BAR,
    COMMA,
    DASH,
    PLUS,
    STAR,
    SLASH,
    DOT,

    AMPERSAND_AMPERSAND,
    LEFT_ANGLE_EQUALS,
    RIGHT_ANGLE_EQUALS,
    EQUALS_EQUALS,
    DASH_ARROW,
    BAR_BAR,

    DOT_DOT_DOT,

    LITERAL,
    NAME_REF,
    PREFIX_EXPR,
    BIN_EXPR,
    PAREN_EXPR,
    BLOCK_EXPR,
    RETURN_EXPR,
    CALL_EXPR,
    INDEX_EXPR,
    IF_EXPR,
    LOOP_EXPR,

    TYPE_KW,
    FN_KW,
    LET_KW,
    RETURN_KW,
    IF_KW,
    ELSE_KW,
    LOOP_KW,
    BREAK_KW,
}
use SyntaxKind::*;

pub enum Subtokens {
    One(SyntaxKind),
    Two(SyntaxKind, SyntaxKind),
    Three(SyntaxKind, SyntaxKind, SyntaxKind),
}

impl Subtokens {
    pub fn number(&self) -> usize {
        match *self {
            Self::One(..) => 1,
            Self::Two(..) => 2,
            Self::Three(..) => 3,
        }
    }
}

impl SyntaxKind {
    pub fn is_trivia(&self) -> bool {
        match self {
            WHITESPACE => true,
            _ => false,
        }
    }
    pub fn subtokens(&self) -> Subtokens {
        match *self {
            EQUALS_EQUALS => Subtokens::Two(EQUALS, EQUALS),
            AMPERSAND_AMPERSAND => Subtokens::Two(AMPERSAND, AMPERSAND),
            LEFT_ANGLE_EQUALS => Subtokens::Two(LEFT_ANGLE, EQUALS),
            RIGHT_ANGLE_EQUALS => Subtokens::Two(RIGHT_ANGLE, EQUALS),
            BAR_BAR => Subtokens::Two(BAR, BAR),
            DASH_ARROW => Subtokens::Two(DASH, RIGHT_ANGLE),
            DOT_DOT_DOT => Subtokens::Three(DOT, DOT, DOT),
            token => Subtokens::One(token),
        }
    }
    pub fn terminated_by_semicolon(&self) -> bool {
        match *self {
            LITERAL | NAME_REF | PREFIX_EXPR | BIN_EXPR | PAREN_EXPR | BLOCK_EXPR | RETURN_EXPR
            | CALL_EXPR => true,
            IF_EXPR | LOOP_EXPR => false,
            _ => unreachable!(),
        }
    }
}

#[macro_export]
macro_rules! T {
    ('(') => {
        crate::SyntaxKind::LEFT_PAREN
    };
    (')') => {
        crate::SyntaxKind::RIGHT_PAREN
    };
    ('{') => {
        crate::SyntaxKind::LEFT_CURLY
    };
    ('}') => {
        crate::SyntaxKind::RIGHT_CURLY
    };
    ('[') => {
        crate::SyntaxKind::LEFT_SQUARE
    };
    (']') => {
        crate::SyntaxKind::RIGHT_SQUARE
    };
    (<) => {
        crate::SyntaxKind::LEFT_ANGLE
    };
    (>) => {
        crate::SyntaxKind::RIGHT_ANGLE
    };
    (:) => {
        crate::SyntaxKind::COLON
    };
    (;) => {
        crate::SyntaxKind::SEMICOLON
    };
    (&) => {
        crate::SyntaxKind::AMPERSAND
    };
    (=) => {
        crate::SyntaxKind::EQUALS
    };
    (|) => {
        crate::SyntaxKind::BAR
    };
    (,) => {
        crate::SyntaxKind::COMMA
    };
    (-) => {
        crate::SyntaxKind::DASH
    };
    (+) => {
        crate::SyntaxKind::PLUS
    };
    (*) => {
        crate::SyntaxKind::STAR
    };
    (/) => {
        crate::SyntaxKind::SLASH
    };
    (.) => {
        crate::SyntaxKind::DOT
    };
    (&&) => {
        crate::SyntaxKind::AMPERSAND_AMPERSAND
    };
    (==) => {
        crate::SyntaxKind::EQUALS_EQUALS
    };
    (>=) => {
        crate::SyntaxKind::RIGHT_ANGLE_EQUALS
    };
    (<=) => {
        crate::SyntaxKind::LEFT_ANGLE_EQUALS
    };
    (||) => {
        crate::SyntaxKind::BAR_BAR
    };
    (->) => {
        crate::SyntaxKind::DASH_ARROW
    };
    (...) => {
        crate::SyntaxKind::DOT_DOT_DOT
    };
    (type) => {
        crate::SyntaxKind::TYPE_KW
    };
    (fn) => {
        crate::SyntaxKind::FN_KW
    };
    (let) => {
        crate::SyntaxKind::LET_KW
    };
    (return) => {
        crate::SyntaxKind::RETURN_KW
    };
    (if) => {
        crate::SyntaxKind::IF_KW
    };
    (else) => {
        crate::SyntaxKind::ELSE_KW
    };
    (loop) => {
        crate::SyntaxKind::LOOP_KW
    };
}
