#[allow(non_camel_case_types)]
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum SyntaxKind {
    TOMBSTONE,
    ERROR,

    EOF,
    EOL,

    MODULE,

    NAME,
    SCOPED_NAME,

    BASIC_TYPE,
    POINTER_TYPE,

    PARAM_LIST,
    PARAM,
    VA_PARAM,

    LET_ITEM,
    FN_ITEM,
    EXPR_ITEM,
    TYPE_ITEM,
    IMPORT_ITEM,

    TYPE_MEMBER,

    IDENT,
    WHITESPACE,
    COMMENT,

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
    BANG,

    AMPERSAND_AMPERSAND,
    BAR_BAR,
    LEFT_ANGLE_EQUALS,
    RIGHT_ANGLE_EQUALS,
    EQUALS_EQUALS,
    BANG_EQUALS,
    DASH_ARROW,
    COLON_COLON,

    DOT_DOT_DOT,

    LITERAL,
    STRING,
    NUMBER,
    STRUCT_LITERAL,

    NAME_REF,
    PREFIX_EXPR,
    BIN_EXPR,
    PAREN_EXPR,
    BLOCK_EXPR,
    RETURN_EXPR,
    CALL_EXPR,
    INDEX_EXPR,
    AS_EXPR,
    IF_EXPR,
    LOOP_EXPR,
    WHILE_EXPR,
    BREAK_EXPR,
    CONTINUE_EXPR,

    AS_KW,
    BREAK_KW,
    CONTINUE_KW,
    ELSE_KW,
    EXTERN_KW,
    FN_KW,
    IF_KW,
    IMPORT_KW,
    LET_KW,
    LOOP_KW,
    WHILE_KW,
    MOD_KW,
    RETURN_KW,
    TYPE_KW,
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
    (!) => {
        crate::SyntaxKind::BANG
    };
    (&&) => {
        crate::SyntaxKind::AMPERSAND_AMPERSAND
    };
    (==) => {
        crate::SyntaxKind::EQUALS_EQUALS
    };
    (!=) => {
        crate::SyntaxKind::BANG_EQUALS
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
    (::) => {
        crate::SyntaxKind::COLON_COLON
    };
    (...) => {
        crate::SyntaxKind::DOT_DOT_DOT
    };
    (mod) => {
        crate::SyntaxKind::MOD_KW
    };
    (import) => {
        crate::SyntaxKind::IMPORT_KW
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
    (while) => {
        crate::SyntaxKind::WHILE_KW
    };
    (break) => {
        crate::SyntaxKind::BREAK_KW
    };
    (continue) => {
        crate::SyntaxKind::CONTINUE_KW
    };
    (as) => {
        crate::SyntaxKind::AS_KW
    };
    (extern) => {
        crate::SyntaxKind::EXTERN_KW
    };
}

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
    pub fn is_keyword(&self) -> bool {
        match *self {
            Self::MOD_KW
            | Self::IMPORT_KW
            | Self::TYPE_KW
            | Self::FN_KW
            | Self::LET_KW
            | Self::RETURN_KW
            | Self::IF_KW
            | Self::ELSE_KW
            | Self::LOOP_KW
            | Self::WHILE_KW
            | Self::BREAK_KW
            | Self::CONTINUE_KW
            | Self::AS_KW
            | Self::EXTERN_KW => true,
            _ => false,
        }
    }

    pub fn is_operator(&self) -> bool {
        match *self {
            T![<]
            | T![>]
            | T![&]
            | T![=]
            | T![|]
            | T![-]
            | T![+]
            | T![*]
            | T![/]
            | T![.]
            | T![&&]
            | T![||]
            | T![<=]
            | T![>=]
            | T![==]
            | T![!=]
            | T![::]
            | T![->]
            | T![...] => true,
            _ => false,
        }
    }

    pub fn is_trivia(&self) -> bool {
        match self {
            Self::EOL | Self::WHITESPACE | Self::COMMENT => true,
            _ => false,
        }
    }

    pub fn subtokens(&self) -> Subtokens {
        use Subtokens::*;
        match *self {
            T![==] => Two(T![=], T![=]),
            T![!=] => Two(T![!], T![=]),
            T![&&] => Two(T![&], T![&]),
            T![<=] => Two(T![<], T![=]),
            T![>=] => Two(T![>], T![=]),
            T![||] => Two(T![|], T![|]),
            T![->] => Two(T![-], T![>]),
            T![::] => Two(T![:], T![:]),
            T![...] => Three(T![.], T![.], T![.]),
            _ => One(*self),
        }
    }

    pub fn terminated_by_semicolon(&self) -> bool {
        match *self {
            Self::LITERAL
            | Self::NAME_REF
            | Self::PREFIX_EXPR
            | Self::BIN_EXPR
            | Self::PAREN_EXPR
            | Self::BLOCK_EXPR
            | Self::RETURN_EXPR
            | Self::INDEX_EXPR
            | Self::CALL_EXPR
            | Self::BREAK_EXPR
            | Self::CONTINUE_EXPR => true,
            Self::IF_EXPR | Self::LOOP_EXPR | Self::WHILE_EXPR => false,
            _ => unreachable!(),
        }
    }
}
