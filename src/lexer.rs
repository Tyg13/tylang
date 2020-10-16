use crate::util::intern_map::InternMap;
use crate::util::{Anchor, Source, Span};
use itertools::Itertools;

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

                    Invalid => "#?INVALID?#",
                }
            }
        }
        fn keyword<T: AsRef<str>>(identifier: T) -> Option<TokenKind> {
            match identifier.as_ref() {
                $($keyword_repr => Some(TokenKind::$keyword),)*
                _ => None,
            }
        }
        fn special_repr(kind: TokenKind) -> Option<String> {
            match kind {
                $(TokenKind::$keyword => Some(String::from($keyword_repr)),)*
                $(TokenKind::$single => Some(String::from($single_repr)),)*
                $(TokenKind::$double => Some(String::from($double_repr)),)*
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
        LeftBrace => "{",
        RightBrace => "}",
        LeftBracket => "[",
        RightBracket => "]",
        LeftParen => "(",
        RightParen => ")",
    }
    double => {
        AmpAmp => "&&",
        BarBar => "||",
        ColonColon => "::",
        BangEquals => "!=",
        EqualsEquals => "==",
        MinusArrow => "->",
        EqualsArrow => "=>",
        LeftAngleEquals => "<=",
        RightAngleEquals => ">=",
    }
}

// An edit is a range which is to be modified, along with a string corresponding
// to content up to but no greater in length than the range specified.
//
// To service an edit, you need to know the current state of the tree, but also
// we need to know how much of the tree is going to stay after the edit.
//
// When we service an edit, we'll basically need to spin up a lexer that takes
// in only a chunk of the source as its input. The tricky part is taking this
// new token tree, the old token tree, and synthesizing a modified tree.

// Edit the token stream as follows
//
// If we have an Edit containing some String at location (line, col)
//  - Determine the location of the edit

fn edit(edit: Edit, old: &TokenMap) -> TokenMap {
    todo!()
}

pub fn lex(source: &Source) -> TokenMap {
    Lexer::new(source).lex()
}

struct Edit {
    text: String,
    span: Span,
}

impl Edit {
    fn new(text: &str, position: Anchor) -> Self {
        let start = position;
        let end = {
            let mut end = start;
            for c in text.chars() {
                if c == '\n' {
                    end.column = 1;
                    end.line += 1;
                } else {
                    end.column += 1;
                }
            }
            end
        };
        Self {
            text: text.to_string(),
            span: Span { start, end },
        }
    }
}

struct Lexer<'source> {
    source: &'source Source,
    map: TokenMap,
    text_pos: Anchor,
    char_pos: usize,
}

impl<'source> Lexer<'source> {
    fn new(source: &'source Source) -> Self {
        Self {
            source,
            map: TokenMap::new(),
            text_pos: Anchor { column: 1, line: 1 },
            char_pos: 0,
        }
    }

    fn lex(mut self) -> TokenMap {
        self.scan_tokens();
        self.map
    }

    /// Scan tokens
    fn scan_tokens(&mut self) {
        while self.peek().is_some() {
            let token = self.scan_token();
            self.map.tokens.push(token);
        }
    }

    /// Scan a token
    /// ```
    /// Token ::= Double | Single | String | Comment | Whitespace
    ///         | Number | Character | Identifier | Invalid
    /// ```
    fn scan_token(&mut self) -> GreenToken {
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
                    GreenToken {
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
    fn scan_string(&mut self) -> GreenToken {
        self.advance(1);
        let literal: String = self.peeking_take_while(|&c| c != '"').collect();
        self.next()
            .unwrap_or_else(|| self.err("EOF while scanning string literal!"));
        let id = self.map.strings.add(literal);
        GreenToken {
            kind: TokenKind::String,
            meta: Meta::String(id),
        }
    }

    /// Scan a comment
    /// ```
    /// Comment ::= LineComment | BlockComment
    /// ```
    fn scan_comment(&mut self) -> GreenToken {
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
    fn scan_block_comment(&mut self) -> GreenToken {
        self.advance(1);
        let mut text = String::new();
        let mut last_was_star = false;
        loop {
            let c = self
                .next()
                .unwrap_or_else(|| self.err("EOF while scanning block comment"));
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
        GreenToken {
            kind: TokenKind::Comment,
            meta: Meta::Comment(id),
        }
    }

    /// Scan a line comment
    /// ```
    /// LineComment ::= "//" {character - '\n'} '\n'
    /// ```
    fn scan_line_comment(&mut self) -> GreenToken {
        self.advance(1);
        let text: String = self.peeking_take_while(|&c| c != '\n').collect();
        if let Some('\n') = self.peek() {
            self.advance(1);
        }
        let comment = Comment {
            kind: CommentKind::Line,
            text,
        };
        let id = self.map.comments.add(comment);
        GreenToken {
            kind: TokenKind::Comment,
            meta: Meta::Comment(id),
        }
    }

    /// Scan character
    /// ```
    /// Character ::= "'" character "'"
    /// ```
    fn scan_character(&mut self) -> GreenToken {
        self.advance(1);
        let literal: Vec<char> = self.peeking_take_while(|&c| c != '\'').collect();
        self.next()
            .unwrap_or_else(|| self.err("EOF while scanning char literal"));
        let character = match literal.as_slice() {
            &[] => self.err("Empty character literal!"),
            &[c] => c,
            &[..] => self.err("Oversize character literal!"),
        };
        let number = Number {
            value: character as usize,
            parity: Parity::Unsigned,
        };
        let id = self.map.numbers.add(number);
        GreenToken {
            kind: TokenKind::Character,
            meta: Meta::Character(id),
        }
    }

    /// Scan whitespace
    /// ```
    /// Whitespace ::= ascii_whitespace
    /// ```
    fn scan_whitespace(&mut self) -> GreenToken {
        let whitespace: String = self
            .peeking_take_while(|c| c.is_ascii_whitespace())
            .collect();
        let id = self.map.whitespace.add(whitespace);
        GreenToken {
            kind: TokenKind::Whitespace,
            meta: Meta::Whitespace(id),
        }
    }

    /// Scan number
    /// ```
    /// Number ::= ["-"] {digit}
    /// ```
    fn scan_number(&mut self) -> GreenToken {
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
        GreenToken {
            kind: TokenKind::Number,
            meta: Meta::Number(id),
        }
    }

    /// Scan identifier or keyword
    /// ```
    /// Identifier ::= {alphanum | "_"} ["!" | "?"]
    /// ```
    fn scan_identifier_or_keyword(&mut self) -> GreenToken {
        let mut ident: String = self.peeking_take_while(|&c| Lexer::ident(c)).collect();
        if let Some(c) = self.peek() {
            if c == '!' || c == '?' {
                self.advance(1);
                ident.push(c);
            }
        }
        let (kind, meta) = match keyword(&ident) {
            Some(keyword) => (keyword, Meta::None),
            None => {
                let id = self.map.idents.add(ident);
                (TokenKind::Identifier, Meta::Identifier(id))
            }
        };
        GreenToken { kind, meta }
    }

    fn try_scan_single(&mut self) -> Option<GreenToken> {
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
            '{' => LeftBrace,
            '}' => RightBrace,
            '[' => LeftBracket,
            ']' => RightBracket,
            '(' => LeftParen,
            ')' => RightParen,
            '-' if !start_of_numeric_literal(self.look(Ahead::One)) => Minus,
            '/' if !start_of_comment(self.look(Ahead::One)) => Slash,
            _ => return None,
        };
        self.advance(1);
        Some(GreenToken {
            kind,
            meta: Meta::None,
        })
    }

    fn try_scan_double(&mut self) -> Option<GreenToken> {
        use TokenKind::*;
        macro_rules! double {
            ($(($first:literal, $second:literal) => $double:ident,)*) => {
                match self.peek()? {
                    $($first if $second == self.look(Ahead::One)? => {
                        self.advance(2);
                        Some(GreenToken {
                            kind: $double,
                            meta: Meta::None,
                        })
                    })*
                    _ => None
                }
            }
        }
        double! {
            ('!', '=') => BangEquals,
            ('=', '=') => EqualsEquals,
            (':', ':') => ColonColon,
            ('|', '|') => BarBar,
            ('&', '&') => AmpAmp,
            ('-', '>') => MinusArrow,
            ('=', '>') => EqualsArrow,
            ('<', '=') => LeftAngleEquals,
            ('>', '=') => RightAngleEquals,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.char(self.char_pos).cloned()
    }

    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            if let Some('\n') = self.peek() {
                self.text_pos.column = 1;
                self.text_pos.line += 1;
            } else {
                self.text_pos.column += 1;
            }
            self.char_pos += 1;
        }
    }

    fn look(&self, n: Ahead) -> Option<char> {
        self.source.char(self.char_pos + n.times()).cloned()
    }

    fn err<T: AsRef<str>>(&self, msg: T) -> ! {
        panic!(
            "{msg}\n{context}",
            msg = msg.as_ref(),
            context = self
                .source
                .give_context_at(self.text_pos)
                .unwrap_or_default(),
        )
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
    tokens: Vec<GreenToken>,
    idents: InternMap<IdentId, String>,
    numbers: InternMap<NumberId, Number>,
    strings: InternMap<StringId, String>,
    comments: InternMap<CommentId, Comment>,
    whitespace: InternMap<WhitespaceId, String>,
}

impl TokenMap {
    fn new() -> Self {
        Self {
            tokens: Vec::new(),
            idents: InternMap::new(),
            numbers: InternMap::new(),
            strings: InternMap::new(),
            comments: InternMap::new(),
            whitespace: InternMap::new(),
        }
    }

    pub fn tokens(&self) -> Tokens<'_> {
        let mut position = Anchor { line: 1, column: 1 };
        self.tokens
            .iter()
            .enumerate()
            .map(|(index, token)| {
                let token = Token {
                    map: self,
                    inner: token,
                    position,
                    index,
                };
                position = token.span().end;
                token
            })
            .collect()
    }

    fn ident(&self, token: &GreenToken) -> Option<&String> {
        match token.meta {
            Meta::Identifier(id) => self.idents.get(&id),
            _ => None,
        }
    }

    fn num(&self, token: &GreenToken) -> Option<&Number> {
        match token.meta {
            Meta::Number(id) => self.numbers.get(&id),
            _ => None,
        }
    }

    fn char(&self, token: &GreenToken) -> Option<&Number> {
        match token.meta {
            Meta::Character(id) => self.numbers.get(&id),
            _ => None,
        }
    }

    fn string(&self, token: &GreenToken) -> Option<&String> {
        match token.meta {
            Meta::String(id) => self.strings.get(&id),
            _ => None,
        }
    }

    fn comment(&self, token: &GreenToken) -> Option<&Comment> {
        match token.meta {
            Meta::Comment(id) => self.comments.get(&id),
            _ => None,
        }
    }

    fn whitespace(&self, token: &GreenToken) -> Option<&String> {
        match token.meta {
            Meta::Whitespace(id) => self.whitespace.get(&id),
            _ => None,
        }
    }
}

impl Token<'_> {
    pub fn as_num(&self) -> Option<Number> {
        self.map.num(self.inner).cloned()
    }

    pub fn as_ident(&self) -> Option<String> {
        self.map.ident(self.inner).cloned()
    }

    pub fn position(&self) -> Anchor {
        self.position
    }

    pub fn kind(&self) -> TokenKind {
        self.inner.kind
    }

    pub fn span(&self) -> Span {
        let start = self.position();
        let mut end = start;
        for c in self.repr().chars() {
            if c == '\n' {
                end.line += 1;
                end.column = 1;
            } else {
                end.column += 1;
            }
        }
        Span { start, end }
    }

    pub fn repr(&self) -> String {
        let inner = self.inner;
        match inner.kind {
            TokenKind::Identifier => self.as_ident(),
            TokenKind::Character => self
                .map
                .char(inner)
                .map(|c| std::char::from_u32(c.value as u32).unwrap().to_string()),
            TokenKind::String => self.map.string(inner).cloned(),
            TokenKind::Comment => self.map.comment(inner).map(|comment| {
                let text = comment.text.clone();
                match comment.kind {
                    CommentKind::Block => format!("/*{}*/", text),
                    CommentKind::Line => format!("//{}\n", text),
                }
            }),
            TokenKind::Whitespace => self.map.whitespace(inner).cloned(),
            TokenKind::Number => self.as_num().map(|num| {
                let value = num.value.to_string();
                let signed = num.parity == Parity::Signed;
                format!(
                    "{sign}{value}",
                    sign = if signed { "-" } else { "" },
                    value = value
                )
            }),
            kind => special_repr(kind),
        }
        .expect("Internal token map inconsistency")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tokens<'map> {
    inner: Vec<Token<'map>>,
}

impl Tokens<'_> {
    pub fn strip_comments_and_whitespace(self) -> Self {
        let inner = self
            .into_iter()
            .filter(|token| {
                token.kind() != TokenKind::Comment && token.kind() != TokenKind::Whitespace
            })
            .collect();
        Self { inner }
    }
}

impl<'map> std::ops::Deref for Tokens<'map> {
    type Target = Vec<Token<'map>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'map> std::iter::IntoIterator for Tokens<'map> {
    type Item = Token<'map>;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'map> std::iter::FromIterator<Token<'map>> for Tokens<'map> {
    fn from_iter<I: IntoIterator<Item = Token<'map>>>(iter: I) -> Self {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct RedToken<'map> {
    map: &'map TokenMap,
    inner: &'map GreenToken,
    position: Anchor,
    index: usize,
}

pub type Token<'map> = RedToken<'map>;

impl std::fmt::Debug for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Token")
            .field("kind", &self.inner.kind)
            .field("repr", &self.repr())
            .field("position", &self.position)
            .field("index", &self.index)
            .finish()
    }
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.repr())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct GreenToken {
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
    pub text: String,
    pub kind: CommentKind,
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

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::util::SourceBuilder;

    pub fn lex(source: &str) -> TokenMap {
        super::lex(&SourceBuilder::new().source(source).build())
    }

    pub fn edit(tree: &TokenMap, input: &str, (line, column): (usize, usize)) -> TokenMap {
        super::edit(Edit::new(input, Anchor { line, column }), tree)
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
        },
    }

    fn token_map(test_tokens: &[TestToken]) -> TokenMap {
        let mut idents = InternMap::new();
        let mut numbers = InternMap::new();
        let mut strings = InternMap::new();
        let mut comments = InternMap::new();
        let mut whitespace = InternMap::new();
        let mut tokens = Vec::new();
        for token in test_tokens {
            tokens.push(match token {
                TestToken::Identifier(ident) => {
                    let id = idents.add(ident.to_string());
                    GreenToken {
                        kind: TokenKind::Identifier,
                        meta: Meta::Identifier(id),
                    }
                }
                TestToken::Character(c) => {
                    let number = Number {
                        value: *c as usize,
                        parity: Parity::Unsigned,
                    };
                    let id = numbers.add(number);
                    GreenToken {
                        kind: TokenKind::Character,
                        meta: Meta::Character(id),
                    }
                }
                TestToken::String(s) => {
                    let id = strings.add(s.to_string());
                    GreenToken {
                        kind: TokenKind::String,
                        meta: Meta::String(id),
                    }
                }
                TestToken::Number(num, parity) => {
                    let number = Number {
                        value: *num,
                        parity: *parity,
                    };
                    let id = numbers.add(number);
                    GreenToken {
                        kind: TokenKind::Number,
                        meta: Meta::Number(id),
                    }
                }
                TestToken::Comment { kind, text } => {
                    let comment = Comment {
                        kind: *kind,
                        text: text.to_string(),
                    };
                    let id = comments.add(comment);
                    GreenToken {
                        kind: TokenKind::Comment,
                        meta: Meta::Comment(id),
                    }
                }
                TestToken::Whitespace(s) => {
                    let id = whitespace.add(s.to_string());
                    GreenToken {
                        kind: TokenKind::Whitespace,
                        meta: Meta::Whitespace(id),
                    }
                }
                TestToken::Other { kind } => GreenToken {
                    kind: *kind,
                    meta: Meta::None,
                },
            });
        }
        TokenMap {
            tokens,
            idents,
            numbers,
            strings,
            comments,
            whitespace,
        }
    }

    fn ident(s: &'static str) -> TestToken {
        TestToken::Identifier(s)
    }

    fn unsigned(n: usize) -> TestToken {
        TestToken::Number(n, Parity::Unsigned)
    }

    fn signed(n: isize) -> TestToken {
        TestToken::Number(n.abs() as usize, Parity::Signed)
    }

    fn single(kind: TokenKind) -> TestToken {
        TestToken::Other { kind }
    }

    fn double(kind: TokenKind) -> TestToken {
        TestToken::Other { kind }
    }

    fn string(s: &'static str) -> TestToken {
        TestToken::String(s)
    }

    fn comment(kind: CommentKind, text: &'static str) -> TestToken {
        TestToken::Comment { kind, text }
    }

    fn s(whitespace: &'static str) -> TestToken {
        TestToken::Whitespace(whitespace)
    }

    fn ch(character: char) -> TestToken {
        TestToken::Character(character)
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
    fn characters() {
        assert_eq!(
            lex(r#"'c''?' '-''1' ',''"'"#),
            token_map(&[
                ch('c'),
                ch('?'),
                s(" "),
                ch('-'),
                ch('1'),
                s(" "),
                ch(','),
                ch('"')
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
                comment(Block, " desu "),
                comment(Line, "te'bayo")
            ])
        );
    }

    #[test]
    fn singles() {
        use TokenKind::*;
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
                single(LeftParen),
                single(RightParen),
                single(LeftBrace),
                single(RightBrace),
                single(LeftBracket),
                single(RightBracket),
                single(Slash)
            ])
        );
    }

    #[test]
    fn doubles() {
        use TokenKind::*;
        assert_eq!(
            lex("!===::||&&->=><=>="),
            token_map(&[
                double(BangEquals),
                double(EqualsEquals),
                double(ColonColon),
                double(BarBar),
                double(AmpAmp),
                double(MinusArrow),
                double(EqualsArrow),
                double(LeftAngleEquals),
                double(RightAngleEquals),
            ])
        );
    }

    #[test]
    fn mixed_singles_doubles() {
        use TokenKind::*;
        assert_eq!(
            lex("!!====-->=>="),
            token_map(&[
                single(Bang),
                double(BangEquals),
                double(EqualsEquals),
                single(Equals),
                single(Minus),
                double(MinusArrow),
                double(EqualsArrow),
                single(Equals),
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
        assert_eq!(map.whitespace.len(), 1);
    }

    //#[test]
    //fn editing() {
    //    assert_eq!(lex(" 123 4"), edit(&lex("123 4"), " ", (1, 1)));
    //}

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
