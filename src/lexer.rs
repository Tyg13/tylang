use crate::util::intern_map::InternMap;
use crate::util::{Anchor, Source};
use itertools::Itertools;

// A high level overview of what we want
//
// We want a map of tokens that know only their lengths, not their absolute
// position. This allows us to insert into the middle of a map and not have to
// recompute the positions of the tokens that follow. Certain insertion events
// will still require us to toss out the subsequent tokens (like the insertion
// of a `"`" or `'`)

// An edit is a range which is to be modified, along with a string corresponding
// to content up to but no greater in length than the range specified.
//
// To service an edit, you need to know the current state of the tree, but also
// we need to know how much of the tree is going to stay after the edit.
//
// When we service an edit, we'll basically need to spin up a lexer that takes
// in only a chunk of the source its input. The tricky part is taking this new
// token tree, the old token tree, and synthesizing a modified tree.

// When we record a token, we record its width and height. But we also have to
// record the whitespace. Do we include whitespace tokens, or do we include
// it as metadata?
//
// What are the approaches?
//
// Whitespace tokens =>
// Cons                               Pros
// Parser has to skip over whitespace Possibility for significant whitespace
// (bad?) Excessive tokens?           Simpler merging
//
// Whitespace metadata (store in extra table by token)
// Cons                               Pros
// Complicates merging                Parser doesn't need to know about
// whitespace Waste of memory?
//
// Edit the token stream as follows
//
// If we have an Edit containing some String at location (line, col)
// - lex String into a TokenTree
// - Using the given (current) TokenTree, walk the tree and recreate position
//   info.

macro_rules! declare_token_kinds {
    (keywords => { $($keyword:ident => $keyword_repr:literal,)* }
     single   => { $($single:ident => $single_repr:literal,)* }
     double   => { $($double:ident => $double_repr:literal,)* }) => {
        #[derive(Copy, Clone, Debug, PartialEq)]
        pub enum TokenKind {
            Identifier,
            Number,
            Character,
            String,
            Comment,
            Whitespace,

            $($single,)*
            $($double,)*
            $($keyword,)*

            Invalid,
        }
        impl std::fmt::Display for TokenKind {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                <&str>::from(*self).fmt(f)
            }
        }
        impl From<TokenKind> for &str {
            fn from(token: TokenKind) -> Self {
                use TokenKind::*;
                match token {
                    Identifier => "identifier",
                    Number => "number",
                    Character => "character",
                    String => "string literal",
                    Comment => "comment",
                    Whitespace => "whitespace",

                    $($keyword => $keyword_repr,)*
                    $($single => $single_repr,)*
                    $($double => $double_repr,)*

                    Invalid => "##INVALID##",
                }
            }
        }
        fn keyword<T: AsRef<str>>(identifier: T) -> Option<TokenKind> {
            match identifier.as_ref() {
                $($keyword_repr => Some(TokenKind::$keyword),)*
                _ => None,
            }
        }
    }
}

declare_token_kinds! {
    keywords => {
        Else => "else",
        Fn => "fn",
        If => "if",
        Let => "let",
        Return => "return",
    }
    single => {
        Amp => "&",
        Bang => "!",
        Bar => "|",
        Caret => "^",
        Colon => ":",
        Comma => ",",
        Dot => ".",
        Equals => "=",
        Minus => "-",
        Percent => "%",
        Plus => "+",
        SemiColon => ";",
        Slash => "/",
        Star => "*",
        Tilde => "~",
        Question => "?",

        LeftAngle => "<",
        RightAngle => ">",
    }
    double => {
        AmpAmp => "&&",
        Arrow => "->",
        BarBar => "||",
        ColonColon => "::",
        EqualsEquals => "==",
        LeftAngleEquals => "<=",
        RightAngleEquals => ">=",
    }
}

fn edit(edit: Edit, old: &TokenMap) -> TokenMap {
    todo!()
}

pub fn lex(source: &Source) -> TokenMap {
    Lexer::new(source).lex()
}

struct Edit {
    text: String,
    pos: Anchor,
}

struct Lexer<'source> {
    source: &'source Source,
    map: TokenMap,
    char_pos: usize,
}

impl<'source> Lexer<'source> {
    fn new(source: &'source Source) -> Self {
        Self {
            source,
            map: TokenMap::new(),
            char_pos: 0,
        }
    }

    fn lex(mut self) -> TokenMap {
        // By convention, we denote the top scope as being enclosed by braces
        self.map.tree = self.scan_token_trees(0, TreeKind::Braces);
        self.map
    }

    /// Scan token trees
    /// ```
    /// TokenTrees ::= {TokenTree}
    /// ```
    fn scan_token_trees(&mut self, depth: usize, kind: TreeKind) -> TokenTree {
        let mut children = Vec::new();
        while let Some(tree) = self.scan_one_token_tree(depth, kind) {
            children.push(tree);
        }
        TokenTree::Tree(Tree { kind, children })
    }

    /// Scan a token tree
    /// ```
    /// TokenTree ::= BraceTree | ParenTree | BracketTree | Token
    /// BraceTree   ::= '{' TokenTrees '}'
    /// ParenTree   ::= '(' TokenTrees ')'
    /// BracketTree ::= '[' TokenTrees ']'
    /// ```
    fn scan_one_token_tree(&mut self, depth: usize, kind: TreeKind) -> Option<TokenTree> {
        match self.peek() {
            None if depth != 0 => panic!("unclosed `}`!"),
            None => None,
            Some(c) => match TreeKind::try_from(c) {
                Some((delim_kind, tree_kind)) => {
                    self.advance(1);
                    match delim_kind {
                        DelimKind::Open => Some(self.scan_token_trees(depth + 1, tree_kind)),
                        DelimKind::Close => {
                            if depth == 0 {
                                panic!(
                                    "unexpected `{delim}` at top-level (doesn't close anything)",
                                    delim = c
                                )
                            }
                            if tree_kind != kind {
                                panic!("mismatched brackets/parens/braces!");
                            }
                            None
                        }
                    }
                }
                None => Some(TokenTree::Token(self.scan_token())),
            },
        }
    }

    /// Scan a token
    /// ```
    /// Token ::= Double | Single | String | Comment | Whitespace
    ///         | Number | Character | Identifier | Invalid
    /// ```
    fn scan_token(&mut self) -> Token {
        if let Some(double) = self.try_scan_double() {
            return double;
        }
        if let Some(single) = self.try_scan_single() {
            return single;
        }
        match self.peek() {
            Some(first) => match first {
                '"' => self.scan_string(),
                '/' => self.scan_comment(),
                '\'' => self.scan_character(),
                c if c.is_ascii_whitespace() => self.scan_whitespace(),
                c if c.is_numeric() || c == '-' => self.scan_number(),
                c if Lexer::ident(c) => self.scan_identifier_or_keyword(),
                _ => {
                    self.advance(1);
                    Token {
                        kind: TokenKind::Invalid,
                        meta: Meta::None,
                    }
                }
            },
            None => unreachable!(),
        }
    }

    /// Scan a token
    /// ```
    /// String ::= '"' {character - '"'} '"'
    /// ```
    fn scan_string(&mut self) -> Token {
        self.advance(1);
        let literal: String = self.peeking_take_while(|&c| c != '"').collect();
        let width = literal.len() + 2;
        let height = literal.lines().count();
        self.next().expect("EOF while scanning string literal!");
        let id = self.map.strings.add(literal);
        Token {
            kind: TokenKind::String,
            meta: Meta::String(id),
        }
    }

    /// Scan a comment
    /// ```
    /// Comment ::= LineComment | BlockComment
    /// ```
    fn scan_comment(&mut self) -> Token {
        self.advance(1);
        match self.peek() {
            Some('/') => self.scan_line_comment(),
            Some('*') => self.scan_block_comment(),
            _ => unreachable!(),
        }
    }

    /// Scan a block comment
    /// ```
    /// BlockComment ::= "/*" {character - "*/"} "*/"
    /// ```
    fn scan_block_comment(&mut self) -> Token {
        self.advance(1);
        let mut text = String::new();
        let mut last_was_star = false;
        loop {
            let c = self.next().expect("EOF while scanning block comment");
            if c == '/' && last_was_star {
                // Remove the last star we added
                text.pop();
                break;
            }
            last_was_star = c == '*';
            text.push(c);
        }
        let comment = Comment {
            kind: CommentKind::Block,
            text,
        };
        let id = self.map.comments.add(comment);
        Token {
            kind: TokenKind::Comment,
            meta: Meta::Comment(id),
        }
    }

    /// Scan a line comment
    /// ```
    /// LineComment ::= "//" {character - '\n'} '\n'
    /// ```
    fn scan_line_comment(&mut self) -> Token {
        self.advance(1);
        let text: String = self.peeking_take_while(|&c| c != '\n').collect();
        let comment = Comment {
            kind: CommentKind::Line,
            text,
        };
        let id = self.map.comments.add(comment);
        Token {
            kind: TokenKind::Comment,
            meta: Meta::Comment(id),
        }
    }

    /// Scan character
    /// ```
    /// Character ::= "'" character "'"
    /// ```
    fn scan_character(&mut self) -> Token {
        self.advance(1);
        let literal: Vec<char> = self.peeking_take_while(|&c| c != '\'').collect();
        self.next().expect("EOF while scanning char literal");
        let character = match dbg!(literal.as_slice()) {
            &[] => panic!("Empty character literal!"),
            &[c] => c,
            &[..] => panic!("Oversize character literal!"),
        };
        let number = Number {
            value: character as usize,
            parity: Parity::Unsigned,
        };
        let id = self.map.numbers.add(number);
        Token {
            kind: TokenKind::Character,
            meta: Meta::Character(id),
        }
    }

    /// Scan whitespace
    /// ```
    /// Whitespace ::= ascii_whitespace
    /// ```
    fn scan_whitespace(&mut self) -> Token {
        let whitespace: String = self
            .peeking_take_while(|c| c.is_ascii_whitespace())
            .collect();
        let id = self.map.whitespace.add(whitespace);
        Token {
            kind: TokenKind::Whitespace,
            meta: Meta::Whitespace(id),
        }
    }

    /// Scan number
    fn scan_number(&mut self) -> Token {
        let parity = if let Some('-') = self.peek() {
            self.advance(1);
            Parity::Signed
        } else {
            Parity::Unsigned
        };
        let literal: String = self.peeking_take_while(|c| c.is_numeric()).collect();
        let value = literal.parse().expect("Numeric literal overflows `usize`!");
        let number = Number { value, parity };
        let id = self.map.numbers.add(number);
        Token {
            kind: TokenKind::Number,
            meta: Meta::Number(id),
        }
    }

    fn scan_identifier_or_keyword(&mut self) -> Token {
        let mut ident: String = self.peeking_take_while(|&c| Lexer::ident(c)).collect();
        if let Some(c) = self.peek() {
            if c == '!' || c == '?' {
                self.advance(1);
                ident.push(c);
            }
        }
        let len = ident.len();
        let (kind, meta) = match keyword(&ident) {
            Some(keyword) => (keyword, Meta::None),
            None => {
                let id = self.map.idents.add(ident);
                (TokenKind::Identifier, Meta::Identifier(id))
            }
        };
        Token { kind, meta }
    }

    fn try_scan_single(&mut self) -> Option<Token> {
        fn start_of_comment(next: Option<char>) -> bool {
            next.map(|c| c == '/' || c == '*').unwrap_or(false)
        }
        fn start_of_numeric_literal(next: Option<char>) -> bool {
            next.map(|c| c.is_numeric()).unwrap_or(false)
        }
        use TokenKind::*;
        let kind = match self.peek()? {
            '&' => Amp,
            '|' => Bar,
            '=' => Equals,
            ':' => Colon,
            '^' => Caret,
            '~' => Tilde,
            '?' => Question,
            '.' => Dot,
            ',' => Comma,
            '!' => Bang,
            ';' => SemiColon,
            '*' => Star,
            '%' => Percent,
            '+' => Plus,
            '<' => LeftAngle,
            '>' => RightAngle,
            '-' if !start_of_numeric_literal(self.look(Ahead::One)) => Minus,
            '/' if !start_of_comment(self.look(Ahead::One)) => Slash,
            _ => return None,
        };
        self.advance(1);
        Some(Token {
            kind,
            meta: Meta::None,
        })
    }

    fn try_scan_double(&mut self) -> Option<Token> {
        use TokenKind::*;
        macro_rules! double {
            ($(($first:literal, $second:literal) => $double:ident,)*) => {
                match self.peek()? {
                    $($first if $second == self.look(Ahead::One)? => {
                        self.advance(2);
                        Some(Token {
                            kind: $double,
                            meta: Meta::None,
                        })
                    })*
                    _ => None
                }
            }
        }
        double! {
            ('=', '=') => EqualsEquals,
            (':', ':') => ColonColon,
            ('|', '|') => BarBar,
            ('&', '&') => AmpAmp,
            ('-', '>') => Arrow,
            ('<', '=') => LeftAngleEquals,
            ('>', '=') => RightAngleEquals,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.char(self.char_pos).cloned()
    }

    fn advance(&mut self, n: usize) {
        self.char_pos += n;
    }

    fn look(&self, n: Ahead) -> Option<char> {
        self.source.char(self.char_pos + n.times()).cloned()
    }

    fn ident(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }
}

enum Ahead {
    One,
}

impl Ahead {
    fn times(&self) -> usize {
        match self {
            Ahead::One => 1,
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        let c = self.peek()?;
        self.advance(1);
        Some(c)
    }
}

impl itertools::PeekingNext for Lexer<'_> {
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

#[derive(Clone, Debug, PartialEq)]
pub struct TokenMap {
    pub tree: TokenTree,
    idents: InternMap<IdentId, String>,
    numbers: InternMap<NumberId, Number>,
    strings: InternMap<StringId, String>,
    comments: InternMap<CommentId, Comment>,
    whitespace: InternMap<WhitespaceId, String>,
}

impl TokenMap {
    fn new() -> Self {
        Self {
            tree: TokenTree::Tree(Tree {
                children: Vec::new(),
                kind: TreeKind::Braces,
            }),
            idents: InternMap::new(),
            numbers: InternMap::new(),
            strings: InternMap::new(),
            comments: InternMap::new(),
            whitespace: InternMap::new(),
        }
    }
    pub fn ident(&self, token: &Token) -> Option<&String> {
        match token.meta {
            Meta::Identifier(id) => self.idents.get(&id),
            _ => None,
        }
    }
    pub fn num(&self, token: &Token) -> Option<&Number> {
        match token.meta {
            Meta::Number(id) => self.numbers.get(&id),
            _ => None,
        }
    }
    pub fn char(&self, token: &Token) -> Option<&Number> {
        match token.meta {
            Meta::Character(id) => self.numbers.get(&id),
            _ => None,
        }
    }
    pub fn string(&self, token: &Token) -> Option<&String> {
        match token.meta {
            Meta::String(id) => self.strings.get(&id),
            _ => None,
        }
    }
    pub fn comment(&self, token: &Token) -> Option<&Comment> {
        match token.meta {
            Meta::Comment(id) => self.comments.get(&id),
            _ => None,
        }
    }
    pub fn whitespace(&self, token: &Token) -> Option<&String> {
        match token.meta {
            Meta::Whitespace(id) => self.whitespace.get(&id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenTree {
    Token(Token),
    Tree(Tree),
}

impl TokenTree {
    pub fn tree(&self) -> Option<&Tree> {
        match self {
            TokenTree::Tree(tree) => Some(tree),
            _ => None,
        }
    }
    pub fn token(&self) -> Option<&Token> {
        match self {
            TokenTree::Token(token) => Some(token),
            _ => None,
        }
    }
    pub fn is_comment_or_whitespace(&self) -> bool {
        match self {
            TokenTree::Token(Token {
                kind: TokenKind::Comment | TokenKind::Whitespace,
                ..
            }) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub kind: TreeKind,
    pub children: Vec<TokenTree>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TreeKind {
    Braces,
    Parens,
    Brackets,
}

impl From<TreeKind> for &str {
    fn from(kind: TreeKind) -> Self {
        match kind {
            TreeKind::Braces => "braces",
            TreeKind::Parens => "parens",
            TreeKind::Brackets => "braces",
        }
    }
}

impl std::fmt::Display for TreeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <&str>::from(*self).fmt(f)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum DelimKind {
    Open,
    Close,
}

impl TreeKind {
    fn delim(&self, kind: DelimKind) -> char {
        match kind {
            DelimKind::Open => match self {
                TreeKind::Braces => '{',
                TreeKind::Parens => '(',
                TreeKind::Brackets => '[',
            },
            DelimKind::Close => match self {
                TreeKind::Braces => '}',
                TreeKind::Parens => '}',
                TreeKind::Brackets => ']',
            },
        }
    }
    fn try_from(c: char) -> Option<(DelimKind, TreeKind)> {
        use {DelimKind::*, TreeKind::*};
        match c {
            '{' => Some((Open, Braces)),
            '}' => Some((Close, Braces)),
            '(' => Some((Open, Parens)),
            ')' => Some((Close, Parens)),
            '[' => Some((Open, Brackets)),
            ']' => Some((Close, Brackets)),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub meta: Meta,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Meta {
    Identifier(IdentId),
    Character(NumberId),
    String(StringId),
    Number(NumberId),
    Comment(CommentId),
    Whitespace(WhitespaceId),
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Number {
    pub value: usize,
    pub parity: Parity,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Parity {
    Unsigned,
    Signed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Comment {
    text: String,
    kind: CommentKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommentKind {
    Line,
    Block,
}

use crate::declare_new_intern_id;

declare_new_intern_id!(IdentId);
declare_new_intern_id!(StringId);
declare_new_intern_id!(NumberId);
declare_new_intern_id!(CommentId);
declare_new_intern_id!(WhitespaceId);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Span {
    width: usize,
    height: usize,
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::util::SourceBuilder;

    pub fn lex(source: &str) -> TokenMap {
        super::lex(&SourceBuilder::new().source(source).build())
    }

    #[derive(Copy, Clone, Debug, PartialEq)]
    enum TestToken {
        Identifier(&'static str),
        Character(char),
        String(&'static str),
        Number(usize, Parity),
        Comment {
            kind: CommentKind,
            text: &'static str,
        },
        Whitespace(&'static str),
        Other {
            kind: TokenKind,
            len: usize,
        },
    }

    #[derive(Clone, Debug, PartialEq)]
    enum TestTokenTree {
        Token(TestToken),
        Tree {
            kind: TreeKind,
            children: Vec<TestTokenTree>,
        },
    }

    fn token_map(trees: &[TestTokenTree]) -> TokenMap {
        fn make_subtrees(
            tree: &TestTokenTree,
            idents: &mut InternMap<IdentId, String>,
            numbers: &mut InternMap<NumberId, Number>,
            strings: &mut InternMap<StringId, String>,
            comments: &mut InternMap<CommentId, Comment>,
            whitespace: &mut InternMap<WhitespaceId, String>,
        ) -> TokenTree {
            match tree {
                TestTokenTree::Token(token) => match token {
                    TestToken::Identifier(ident) => {
                        let id = idents.add(ident.to_string());
                        TokenTree::Token(Token {
                            kind: TokenKind::Identifier,
                            meta: Meta::Identifier(id),
                        })
                    }
                    TestToken::Character(c) => {
                        let number = Number {
                            value: *c as usize,
                            parity: Parity::Unsigned,
                        };
                        let id = numbers.add(number);
                        TokenTree::Token(Token {
                            kind: TokenKind::Character,
                            meta: Meta::Character(id),
                        })
                    }
                    TestToken::String(s) => {
                        let id = strings.add(s.to_string());
                        TokenTree::Token(Token {
                            kind: TokenKind::String,
                            meta: Meta::String(id),
                        })
                    }
                    TestToken::Number(num, parity) => {
                        let number = Number {
                            value: *num,
                            parity: *parity,
                        };
                        let id = numbers.add(number);
                        TokenTree::Token(Token {
                            kind: TokenKind::Number,
                            meta: Meta::Number(id),
                        })
                    }
                    TestToken::Comment { kind, text } => {
                        let width = text.len() + 2;
                        let height = match kind {
                            CommentKind::Line => 1,
                            CommentKind::Block => text.lines().count(),
                        };
                        let comment = Comment {
                            kind: *kind,
                            text: text.to_string(),
                        };
                        let id = comments.add(comment);
                        TokenTree::Token(Token {
                            kind: TokenKind::Comment,
                            meta: Meta::Comment(id),
                        })
                    }
                    TestToken::Whitespace(s) => {
                        let width = s.len();
                        let height = s.lines().count();
                        let id = whitespace.add(s.to_string());
                        TokenTree::Token(Token {
                            kind: TokenKind::Whitespace,
                            meta: Meta::Whitespace(id),
                        })
                    }
                    TestToken::Other { kind, len } => TokenTree::Token(Token {
                        kind: *kind,
                        meta: Meta::None,
                    }),
                },
                TestTokenTree::Tree { kind, children } => TokenTree::Tree(Tree {
                    kind: *kind,
                    children: children
                        .iter()
                        .map(|tree| {
                            make_subtrees(tree, idents, numbers, strings, comments, whitespace)
                        })
                        .collect(),
                }),
            }
        }
        use TreeKind::*;
        let mut idents = InternMap::new();
        let mut numbers = InternMap::new();
        let mut strings = InternMap::new();
        let mut comments = InternMap::new();
        let mut whitespace = InternMap::new();
        let children = trees
            .iter()
            .map(|tree| {
                make_subtrees(
                    tree,
                    &mut idents,
                    &mut numbers,
                    &mut strings,
                    &mut comments,
                    &mut whitespace,
                )
            })
            .collect();
        let tree = TokenTree::Tree(Tree {
            kind: Braces,
            children,
        });
        TokenMap {
            tree,
            idents,
            numbers,
            strings,
            comments,
            whitespace,
        }
    }

    fn ident(s: &'static str) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Identifier(s))
    }

    fn unsigned(n: usize) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Number(n, Parity::Unsigned))
    }

    fn signed(n: isize) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Number(n.abs() as usize, Parity::Signed))
    }

    fn single(kind: TokenKind) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Other { kind, len: 1 })
    }

    fn double(kind: TokenKind) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Other { kind, len: 2 })
    }

    fn string(s: &'static str) -> TestTokenTree {
        TestTokenTree::Token(TestToken::String(s))
    }

    fn comment(kind: CommentKind, text: &'static str) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Comment { kind, text })
    }

    fn s(whitespace: &'static str) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Whitespace(whitespace))
    }

    fn ch(character: char) -> TestTokenTree {
        TestTokenTree::Token(TestToken::Character(character))
    }

    fn tree(kind: TreeKind, children: &[TestTokenTree]) -> TestTokenTree {
        TestTokenTree::Tree {
            kind,
            children: children.iter().cloned().collect(),
        }
    }

    #[test]
    fn empty() {
        assert_eq!(lex(""), token_map(&[]));
    }

    #[test]
    fn idents() {
        assert_eq!(
            lex("abced jad"),
            token_map(&[ident("abced"), s(" "), ident("jad")])
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(
            lex("100 200 -100 -200"),
            token_map(&[
                unsigned(100),
                s(" "),
                unsigned(200),
                s(" "),
                signed(-100),
                s(" "),
                signed(-200)
            ])
        );
    }

    #[test]
    fn strings() {
        assert_eq!(
            lex(r#""interior" "crocodile alligator""#),
            token_map(&[string("interior"), s(" "), string("crocodile alligator")])
        );
    }

    #[test]
    fn comments() {
        use CommentKind::*;
        assert_eq!(
            lex("/* watashi wa */ // naruto\n/* desu *///te'bayo"),
            token_map(&[
                comment(Block, " watashi wa "),
                s(" "),
                comment(Line, " naruto"),
                s("\n"),
                comment(Block, " desu "),
                comment(Line, "te'bayo")
            ])
        );
    }

    #[test]
    fn singles() {
        use {TokenKind::*, TreeKind::*};
        assert_eq!(
            lex("&|=-:^~?.,!;*%+<>(){}[]/"),
            token_map(&[
                single(Amp),
                single(Bar),
                single(Equals),
                single(Minus),
                single(Colon),
                single(Caret),
                single(Tilde),
                single(Question),
                single(Dot),
                single(Comma),
                single(Bang),
                single(SemiColon),
                single(Star),
                single(Percent),
                single(Plus),
                single(LeftAngle),
                single(RightAngle),
                tree(Parens, &[]),
                tree(Braces, &[]),
                tree(Brackets, &[]),
                single(Slash)
            ])
        );
    }

    #[test]
    fn doubles() {
        use TokenKind::*;
        assert_eq!(
            lex("==::||&&-><=>="),
            token_map(&[
                double(EqualsEquals),
                double(ColonColon),
                double(BarBar),
                double(AmpAmp),
                double(Arrow),
                double(LeftAngleEquals),
                double(RightAngleEquals)
            ])
        );
    }

    #[test]
    fn mixed_singles_doubles() {
        use TokenKind::*;
        assert_eq!(
            lex("===-->"),
            token_map(&[
                double(EqualsEquals),
                single(Equals),
                single(Minus),
                double(Arrow)
            ])
        );
    }

    #[test]
    fn subtree() {
        use TreeKind::*;
        assert_eq!(
            lex("{jad}is bad"),
            token_map(&[
                tree(Braces, &[ident("jad")]),
                ident("is"),
                s(" "),
                ident("bad")
            ])
        );
    }

    #[test]
    fn interning() {
        let map =
            lex(r#""math city" bitch math "math city" bitch 10 10 10 and 20 equals 50 bitch"#);
        assert_eq!(map.idents.len(), 4);
        assert_eq!(map.strings.len(), 1);
        assert_eq!(map.numbers.len(), 3);
        assert_eq!(map.whitespace.len(), 1);
        let map = lex("/* meow */ /* meow */ // mow\n// mow");
        assert_eq!(map.comments.len(), 2);
        assert_eq!(map.whitespace.len(), 2);
    }

    #[test]
    #[should_panic]
    fn unclosed_delim() {
        lex("[ { jad ");
    }

    #[test]
    #[should_panic(expected = "unexpected `)` at top-level (doesn't close anything)")]
    fn unopened_delim() {
        lex("jad )");
    }

    #[test]
    #[should_panic(expected = "mismatched brackets/parens/braces!")]
    fn mismatched_delims() {
        lex("{ jad ]");
    }

    #[test]
    #[should_panic(expected = "EOF while scanning string literal!")]
    fn non_terminated_string() {
        lex(r#""this is an unterminated string"#);
    }

    #[test]
    #[should_panic(expected = "EOF while scanning block comment")]
    fn non_terminated_block_comment() {
        lex("/* this is an unterminated block comment");
    }

    #[test]
    #[should_panic(expected = "Numeric literal overflows `usize`!")]
    fn overflow_unsigned_numeric_literal() {
        lex("123456789012345678901234567890");
    }

    #[test]
    #[should_panic(expected = "Numeric literal overflows `usize`!")]
    fn overflow_signed_numeric_literal() {
        lex("-123456789012345678901234567890");
    }

    #[test]
    #[should_panic(expected = "EOF while scanning char literal")]
    fn nonterminated_char_literal() {
        lex("'c");
    }

    #[test]
    #[should_panic(expected = "Oversize character literal")]
    fn oversize_char_literal() {
        lex("'ca'");
    }

    #[test]
    #[should_panic(expected = "Empty character literal!")]
    fn empty_char_literal() {
        lex("''");
    }
}
