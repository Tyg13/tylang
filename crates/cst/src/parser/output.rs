use super::*;

#[derive(Debug)]
struct Builder<'source> {
    builder: syntax::Builder,
    token_index: usize,
    position: Position,
    source: &'source str,
    token_lens: Vec<usize>,
    errors: Vec<(String, Position)>,
    context_stack: Vec<SyntaxKind>,
}
impl<'source> EventSink for Builder<'source> {
    fn start_node(&mut self, kind: SyntaxKind) {
        self.context_stack.push(kind);
        self.builder.start_node(kind);
    }

    fn finish_node(&mut self) {
        self.context_stack.pop();
        self.builder.finish_node();
    }

    fn n_tokens(&mut self, kind: SyntaxKind, n: usize) {
        let len = (self.token_index..self.token_index + n)
            .map(|n| self.token_lens[n])
            .sum();
        let text = Self::text_in_range(&self.source, &self.position, len);
        Self::update_pos_from_text(&mut self.position, text);
        if kind != SyntaxKind::ERROR {
            self.builder.token(kind, text);
        }
        self.token_index += n;
    }

    fn error(&mut self, msg: String) {
        let position = self.position;
        if let Some((_, last_position)) = self.errors.last() {
            if *last_position == position {
                return;
            }
        }
        let context = self
            .context_stack
            .last()
            .map(|c| format!(" in {c:?}"))
            .unwrap_or("".to_string());
        self.errors.push((format!("{msg}{context}"), position));
    }
}
impl<'tokens> Builder<'tokens> {
    fn new(
        source: &'tokens str,
        token_lens: Vec<usize>,
        token_cache: crate::lexer::TokenCache,
    ) -> Self {
        Self {
            builder: crate::syntax::Builder::new_with_cache(token_cache),
            token_index: 0,
            position: Position {
                offset: 0,
                line: 1,
                column: 1,
            },
            source,
            token_lens,
            errors: Default::default(),
            context_stack: Default::default(),
        }
    }

    fn text_in_range(source: &'tokens str, position: &Position, len: usize) -> &'tokens str {
        let start = position.offset;
        let end = start + len;
        &source[start..end]
    }

    fn update_pos_from_text(pos: &mut Position, text: &str) {
        for c in text.chars() {
            if c == '\n' {
                pos.line += 1;
                pos.column = 1;
            } else {
                pos.column += 1;
            }
            pos.offset += 1;
        }
    }

    fn finish(self) -> (syntax::Node, Vec<Error>) {
        (
            self.builder.finish(),
            self.errors
                .into_iter()
                .map(|(msg, pos)| Error { msg, pos })
                .collect(),
        )
    }
}

pub(crate) fn parse(mut input: crate::parser::input::Input, entry: grammar::EntryPoint) -> Output {
    let mut builder = Builder::new(input.source, input.token_lens, input.token_cache);
    parse_impl(entry, &mut input.tokens, &mut builder);
    let (root, errors) = builder.finish();
    Output { root, errors }
}

fn parse_impl(
    entry: grammar::EntryPoint,
    token_source: &mut dyn TokenSource,
    event_sink: &mut dyn EventSink,
) {
    let mut events = Parser::new(token_source).parse(entry);
    let mut nodes = Vec::new();
    for i in 0..events.len() {
        match std::mem::replace(&mut events[i], Event::tombstone()) {
            Event::NodeStart(kind, parent) => {
                nodes.push(kind);
                let mut parent = parent;
                while let Some(idx) = parent {
                    // This node's parent is actually further ahead in the event stream.
                    // If we have, for example, a series of nodes, e.g:
                    //
                    //   A: (parent: B)
                    //     ..
                    //   B: (parent: C)
                    //     ..
                    //   C:
                    //     ..
                    //
                    // We want to end up with the following structure:
                    //
                    //   C:
                    //     B:
                    //       A:
                    //        ..
                    //     ..
                    //   ..
                    //
                    // We do so by walking the hierarchy A -> B -> C, and pushing the
                    // events in reverse order (C -> B -> A). As we do so, we replace the
                    // original event with a dead element to mark that it has already
                    // been processed.
                    match std::mem::replace(&mut events[idx], Event::tombstone()) {
                        Event::NodeStart(kind, grandparent) => {
                            nodes.push(kind);
                            parent = grandparent;
                        }
                        _ => panic!("parent {idx} was not a node"),
                    }
                }
                // Now push all non-dead node parents.
                for kind in nodes.drain(..).rev() {
                    if kind != TOMBSTONE {
                        event_sink.start_node(kind);
                    }
                }
            }
            Event::NodeFinish => event_sink.finish_node(),
            Event::Tokens(kind, n) => event_sink.n_tokens(kind, n),
            Event::Error(msg) => event_sink.error(msg),
        }
    }
}

#[derive(Debug)]
pub struct Output {
    pub root: syntax::Node,
    pub errors: Vec<Error>,
}

#[derive(Debug)]
pub struct Error {
    pub msg: String,
    pub pos: Position,
}
