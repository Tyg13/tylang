use crate::green::Subtokens;

use super::*;

pub struct Parser<'tokens> {
    tokens: &'tokens mut dyn TokenSource,
    token_index: usize,
    events: Vec<Event>,
    follow_stack: Vec<HashSet<SyntaxKind>>,
    steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    NodeStart(SyntaxKind, Option<usize>),
    NodeFinish,
    Tokens(SyntaxKind, usize),
    Error(String),
}

impl Event {
    pub fn tombstone() -> Self {
        Event::NodeStart(TOMBSTONE, None)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Marker {
    index: usize,
}

impl Marker {
    pub fn complete(self, parser: &mut Parser<'_>, kind: SyntaxKind) -> CompletedMarker {
        let index = self.index;
        match &mut parser.events[index] {
            event @ Event::NodeStart(TOMBSTONE, None) => {
                *event = Event::NodeStart(kind, None);
            }
            _ => panic!("tried to start a node at non-marker index {index}"),
        }
        parser.finish_node();
        CompletedMarker { index, kind }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedMarker {
    index: usize,
    kind: SyntaxKind,
}

impl CompletedMarker {
    pub fn precede(&self, parser: &mut Parser<'_>) -> Marker {
        let before = parser.start_node();
        let index = self.index;
        match &mut parser.events[index] {
            Event::NodeStart(_, node_start) => {
                *node_start = Some(before.index);
            }
            _ => panic!("tried to precede at non-node index {index}"),
        }
        before
    }

    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }
}

impl<'tokens> Parser<'tokens> {
    pub fn new(tokens: &'tokens mut dyn TokenSource) -> Self {
        Self {
            tokens,
            token_index: 0,
            events: Default::default(),
            follow_stack: vec![Default::default()],
            steps: 0,
        }
    }

    pub fn parse(mut self, entry_point: grammar::EntryPoint) -> Vec<Event> {
        grammar::entry_point(&mut self, entry_point);
        self.events
    }

    pub fn advance(&mut self) {
        if self.peek() == SyntaxKind::EOF {
            self.error("unexpected EOF");
        } else {
            self.token_index += 1;
        }
    }

    pub fn start_node(&mut self) -> Marker {
        let index = self.events.len();
        self.events.push(Event::tombstone());
        Marker { index }
    }

    pub fn node(
        &mut self,
        kind: SyntaxKind,
        inner: impl FnOnce(&mut Self) -> (),
    ) -> CompletedMarker {
        self.advance_to_next_non_trivia();
        let node = self.start_node();
        inner(self);
        node.complete(self, kind)
    }

    pub fn token(&mut self, kind: SyntaxKind) {
        self.step();
        let n_tokens = kind.subtokens().number();
        for _ in 0..n_tokens {
            let kind = self.peek();
            self.remove_follow(kind);
            self.advance();
        }
        self.events.push(Event::Tokens(kind, n_tokens));
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.step();
        self.events.push(Event::Error(msg.into()));
    }

    pub fn unexpected(&mut self, kind: SyntaxKind) {
        self.error(format!("unexpected token {kind:?}"));
        self.skip_until_expected();
    }

    pub fn nth(&self, n: usize) -> SyntaxKind {
        self.tokens.kind_at(n)
    }

    pub fn peek(&self) -> SyntaxKind {
        self.nth(self.token_index)
    }

    pub fn lookahead(&self, n: usize) -> SyntaxKind {
        self.nth(self.token_index + n)
    }

    pub fn maybe(&mut self, kind: SyntaxKind) -> bool {
        self.advance_to_next_non_trivia();
        let found = self.at(kind);
        self.follow_set_mut().remove(&kind);
        found
    }

    pub fn advance_to_next_non_trivia(&mut self) -> SyntaxKind {
        self.eat_trivia();
        self.peek()
    }

    pub fn with_follow_set(&mut self, kinds: &[SyntaxKind], action: impl FnOnce(&mut Self) -> ()) {
        self.follow_stack.push(Default::default());
        self.add_to_follow_set(kinds);
        action(self);
        self.follow_stack.pop();
        assert!(self.follow_stack.len() > 0);
    }

    pub fn remove_follow(&mut self, kind: SyntaxKind) {
        self.follow_set_mut().remove(&kind);
    }

    pub fn add_to_follow_set(&mut self, kinds: &[SyntaxKind]) {
        for kind in kinds {
            self.follow_set_mut().insert(*kind);
        }
    }

    pub fn at(&self, kind: SyntaxKind) -> bool {
        match kind.subtokens() {
            Subtokens::One(t1) => self.lookahead(0) == t1,
            Subtokens::Two(t1, t2) => (self.lookahead(0), self.lookahead(1)) == (t1, t2),
            Subtokens::Three(t1, t2, t3) => {
                (self.lookahead(0), self.lookahead(1), self.lookahead(2)) == (t1, t2, t3)
            }
        }
    }

    pub fn expect_token(&mut self, kind: SyntaxKind) {
        let actual_kind = self.advance_to_next_non_trivia();
        if !self.at(kind) {
            self.error(format!(
                "unexpected token {actual_kind:?} (expected {kind:?})"
            ));
            if let Some(kind) = self.skip_until_expected() {
                self.token(kind);
            }
            self.remove_follow(kind);
        } else {
            self.token(kind);
        }
    }
}

impl Parser<'_> {
    fn step(&mut self) {
        self.steps = self.steps.saturating_add(1);
        //if self.steps >= 10000 {
        //    panic!("parser might be stuck");
        //}
    }

    fn finish_node(&mut self) {
        self.step();
        self.events.push(Event::NodeFinish);
    }

    fn eat_trivia(&mut self) {
        while self.peek().is_trivia() {
            self.token(self.peek());
        }
    }

    fn skip_token(&mut self) {
        self.token(ERROR);
        self.advance();
    }

    fn skip_until_expected(&mut self) -> Option<SyntaxKind> {
        loop {
            match self.advance_to_next_non_trivia() {
                SyntaxKind::EOF => break None,
                kind if self.follow_set().contains(&kind) => break Some(kind),
                _ => self.skip_token(),
            }
        }
    }

    fn follow_set(&self) -> &HashSet<SyntaxKind> {
        self.follow_stack.last().unwrap()
    }

    fn follow_set_mut(&mut self) -> &mut HashSet<SyntaxKind> {
        self.follow_stack.last_mut().unwrap()
    }
}
