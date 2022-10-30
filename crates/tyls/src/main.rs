use std::collections::HashMap;

use crossbeam_channel::Sender;
use crossbeam_queue::ArrayQueue;
use lsp_server::Connection;
use lsp_server::Message;
use lsp_types::ServerCapabilities;
use serde::de::Deserialize;

mod semantic_tokens;

fn start_logging() {
    let log_name = format!("tyls.log");
    simple_logging::log_to_file(log_name, log::LevelFilter::Debug).unwrap();
}

fn notify<N: lsp_types::notification::Notification>(
    sender: &Sender<Message>,
    params: N::Params,
) {
    sender
        .send(Message::Notification(lsp_server::Notification::new(
            N::METHOD.to_string(),
            params,
        )))
        .unwrap();
}

fn parse_module(text: &str) -> cst::syntax::Node {
    cst::parser::parse_str(text).root
}

fn server_caps() -> ServerCapabilities {
    let mut server_caps = ServerCapabilities::default();
    server_caps.text_document_sync =
        Some(lsp_types::TextDocumentSyncCapability::Options({
            let mut options = lsp_types::TextDocumentSyncOptions::default();
            options.open_close = Some(true);
            options.change = Some(lsp_types::TextDocumentSyncKind::FULL);
            options
        }));
    server_caps.semantic_tokens_provider = Some(
        lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions({
            let mut options = lsp_types::SemanticTokensOptions::default();
            options.legend = lsp_types::SemanticTokensLegend::default();
            options.legend.token_types =
                semantic_tokens::legend().types().clone();
            options.full =
                Some(lsp_types::SemanticTokensFullOptions::Bool(true));
            options
        }),
    );
    server_caps.hover_provider =
        Some(lsp_types::HoverProviderCapability::Simple(true));
    server_caps
}

fn initialize_lsp_connection(conn: &Connection) {
    let client_caps = {
        let (id, _) = conn.initialize_start().unwrap();
        let server_info = lsp_types::ServerInfo {
            name: "tyls".to_string(),
            version: None,
        };
        conn.initialize_finish(
            id,
            serde_json::to_value(lsp_types::InitializeResult {
                capabilities: server_caps(),
                server_info: Some(server_info),
            })
            .unwrap(),
        )
        .unwrap();
    };
    log::debug!("{client_caps:#?}");
    info(conn, "tyls initialized");
}

fn compute_semantic_tokens(info: &ModuleInfo) -> Vec<lsp_types::SemanticToken> {
    let (tokens, deltas) = {
        #[derive(Default)]
        struct TokenCollector {
            tokens: Vec<cst::syntax::Token>,
            deltas: Vec<(usize, usize)>,
        }

        impl cst::syntax::traverse::Visitor for TokenCollector {
            fn visit(&mut self, node: cst::syntax::NodeOrToken) {
                if let Some(token) = node.into_token() {
                    self.tokens.push(token.clone());
                    self.deltas.push(delta_position(token.text()));
                }
            }
        }

        fn delta_position(text: &str) -> (usize, usize) {
            let (mut line, mut column) = (0, 0);
            for c in text.chars() {
                if c == '\n' {
                    line += 1;
                    column = 0;
                } else {
                    column += 1;
                }
            }
            (line, column)
        }

        let mut collector = TokenCollector::default();
        collector.deltas.push((0, 0));
        cst::syntax::traverse::preorder(
            &mut collector,
            info.syntax.clone().as_node_or_token(),
        );
        assert_eq!(collector.deltas.len(), collector.tokens.len() + 1);
        (collector.tokens, collector.deltas)
    };

    let mut semantic_tokens = Vec::new();
    let (mut acc_delta_line, mut acc_delta_column) = (0, 0);
    for i in 0..tokens.len() {
        let token = &tokens[i];
        let (delta_line, delta_column) = deltas[i];
        if delta_line > 0 {
            acc_delta_line += delta_line;
            acc_delta_column = delta_column;
        } else {
            acc_delta_column += delta_column;
        }
        use cst::SyntaxKind::*;
        let token_kind = match token.kind() {
            NUMBER => lsp_types::SemanticTokenType::NUMBER,
            STRING => lsp_types::SemanticTokenType::STRING,
            COMMENT => lsp_types::SemanticTokenType::COMMENT,
            IDENT => match token.parent.kind() {
                TYPE_ITEM => lsp_types::SemanticTokenType::STRUCT,
                BASIC_TYPE => lsp_types::SemanticTokenType::TYPE,
                NAME => match token.parent.parent().unwrap().kind() {
                    FN_ITEM => lsp_types::SemanticTokenType::FUNCTION,
                    _ => continue,
                },
                NAME_REF => {
                    let parent = token.parent.clone();
                    let grandparent = parent.parent().unwrap();
                    match (parent.index, grandparent.kind()) {
                        (0, CALL_EXPR) => {
                            lsp_types::SemanticTokenType::FUNCTION
                        }
                        (0, BIN_EXPR) => {
                            if grandparent.child(1).kind() == COLON_COLON {
                                lsp_types::SemanticTokenType::NAMESPACE
                            } else {
                                continue;
                            }
                        }
                        (2, BIN_EXPR) => {
                            if grandparent.parent().unwrap().kind() == CALL_EXPR
                            {
                                lsp_types::SemanticTokenType::FUNCTION
                            } else {
                                continue;
                            }
                        }
                        _ => continue,
                    }
                }
                _ => continue,
            },
            kind if kind.is_operator() => {
                lsp_types::SemanticTokenType::OPERATOR
            }
            kind if kind.is_keyword() => lsp_types::SemanticTokenType::KEYWORD,
            _ => continue,
        };
        semantic_tokens.push(lsp_types::SemanticToken {
            delta_line: acc_delta_line as u32,
            delta_start: acc_delta_column as u32,
            length: token.text().len() as u32,
            token_type: semantic_tokens::legend().idx_of(&token_kind).unwrap(),
            token_modifiers_bitset: 0,
        });
        (acc_delta_line, acc_delta_column) = (0, 0);
    }

    semantic_tokens
}

fn find_syntax_tree_at_position(
    pos: &lsp_types::Position,
    info: &mut ModuleInfo,
) -> Option<String> {
    log::debug!("trying to find node at {pos:?}");
    let lines_to_offsets = info
        .lines_to_offsets
        .retrieve(|| compute_lines_to_offsets(&info.text));
    let offset = (lines_to_offsets[&pos.line] + pos.character) as usize;

    use cst::syntax::traverse::Step;

    let node_at_cursor = cst::syntax::traverse::iterate(
        info.syntax.clone().as_node_or_token(),
        |node| {
            for child in node.children_with_tokens() {
                if child.range().contains(&offset) {
                    return Step::Continue(child.clone());
                }
            }
            Step::Terminate(node)
        },
    );

    let repr = |node: &cst::syntax::NodeOrToken| -> String {
        format!("{}: {:?}", node.index(), node.kind())
    };

    let mut reprs = vec![repr(&node_at_cursor)];
    for parent in node_at_cursor.parents() {
        reprs.push(repr(&parent.clone().into()));
    }

    let mut indent = String::new();
    let tree: Vec<_> = reprs
        .into_iter()
        .rev()
        .map(|repr| {
            let repr = format!("{indent}{}", repr);
            indent.push_str("  ");
            repr
        })
        .collect();

    Some(tree.join("\n"))
}

fn info(conn: &Connection, message: &str) {
    notify::<lsp_types::notification::ShowMessage>(
        &conn.sender,
        lsp_types::ShowMessageParams {
            typ: lsp_types::MessageType::INFO,
            message: message.to_string(),
        },
    );
}

struct ModuleInfo {
    syntax: cst::syntax::Node,
    text: String,
    lines_to_offsets: Provider<HashMap<u32, u32>>,
}

impl ModuleInfo {
    fn new(module: cst::syntax::Node, text: String) -> Self {
        Self {
            syntax: module,
            text,
            lines_to_offsets: Provider::new(),
        }
    }
}

fn compute_lines_to_offsets(text: &str) -> HashMap<u32, u32> {
    let mut line = 0;
    text.char_indices()
        .filter_map(|(idx, c)| {
            if c == '\n' {
                line += 1;
                Some((line, (idx + 1) as u32))
            } else {
                None
            }
        })
        .chain(std::iter::once((0, 0)))
        .collect()
}

struct Provider<T> {
    data: Option<T>,
}

impl<T> Provider<T> {
    fn new() -> Self {
        Self { data: None }
    }

    fn retrieve(&mut self, compute: impl Fn() -> T) -> &T {
        if self.data.is_none() {
            self.data = Some((compute)());
        }
        self.data.as_ref().unwrap()
    }
}

fn main() {
    start_logging();
    let (conn, _threads) = Connection::stdio();
    initialize_lsp_connection(&conn);

    let message_queue: ArrayQueue<Message> = ArrayQueue::new(10);
    crossbeam_utils::thread::scope(|s| {
        let sender_thread = s.spawn(|_| {
            let message_queue = &message_queue;
            loop {
                while let Some(message) = message_queue.pop() {
                    log::debug!("{message:?}");
                    conn.sender.send(message).unwrap();
                }
            }
        });
        let receiver_thread = s.spawn(|_| {
            let mut modules: HashMap<String, ModuleInfo> = HashMap::new();
            loop {
                let msg = conn.receiver.recv().unwrap();
                log::debug!("{msg:?}");
                match msg {
                    Message::Notification(not) => match not.method.as_str() {
                        "textDocument/didOpen" => {
                            let params: lsp_types::DidOpenTextDocumentParams =
                                Deserialize::deserialize(not.params).unwrap();
                            let path = params.text_document.uri.path().to_string();
                            let text = params.text_document.text.to_string();
                            let info = ModuleInfo::new(parse_module(&text), text);
                            modules.insert(path.clone(), info);
                        }
                        "textDocument/didChange" => {
                            let params: lsp_types::DidChangeTextDocumentParams =
                                Deserialize::deserialize(not.params).unwrap();
                            let path = params.text_document.uri.path().to_string();
                            let text = params.content_changes[0].text.to_string();
                            modules
                                .insert(path.clone(), ModuleInfo::new(parse_module(&text), text));
                        }
                        "textDocument/didClose" => {
                            let params: lsp_types::DidCloseTextDocumentParams =
                                Deserialize::deserialize(not.params).unwrap();
                            let path = params.text_document.uri.path().to_string();
                            modules.remove(&path);
                        }
                        _ => {}
                    },
                    Message::Request(req) => match req.method.as_str() {
                        "textDocument/hover" => {
                            let params: lsp_types::HoverParams =
                                Deserialize::deserialize(req.params).unwrap();
                            let path = params
                                .text_document_position_params
                                .text_document
                                .uri
                                .path();
                            if let Some(module) = modules.get_mut(path) {
                                let pos = params.text_document_position_params.position;
                                let result =
                                    find_syntax_tree_at_position(&pos, module).map(|kind| {
                                        serde_json::to_value(lsp_types::Hover {
                                            contents: lsp_types::HoverContents::Markup(
                                                lsp_types::MarkupContent {
                                                    kind: lsp_types::MarkupKind::PlainText,
                                                    value: kind,
                                                },
                                            ),
                                            range: None,
                                        })
                                        .unwrap()
                                    });
                                let message = Message::Response(lsp_server::Response {
                                    id: req.id,
                                    result,
                                    error: None,
                                });
                                message_queue.push(message).unwrap();
                            }
                        }
                        "textDocument/semanticTokens/full" => {
                            let params: lsp_types::SemanticTokensParams =
                                Deserialize::deserialize(req.params).unwrap();
                            let path = params.text_document.uri.path();
                            if let Some(module) = modules.get(path) {
                                let tokens = compute_semantic_tokens(&module);
                                let message = Message::Response(lsp_server::Response {
                                    id: req.id,
                                    result: Some(
                                        serde_json::to_value(lsp_types::SemanticTokens {
                                            result_id: None,
                                            data: tokens,
                                        })
                                        .unwrap(),
                                    ),
                                    error: None,
                                });
                                message_queue.push(message).unwrap();
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        });
        sender_thread.join().unwrap();
        receiver_thread.join().unwrap();
    })
    .unwrap();
}
