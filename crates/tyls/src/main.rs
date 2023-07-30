use std::collections::HashMap;

use crossbeam_channel::Sender;
use crossbeam_queue::ArrayQueue;
use lsp_server::Connection;
use lsp_server::Message;
use lsp_types::ServerCapabilities;
use parser::Output;
use serde::de::Deserialize;

mod semantic_tokens;

fn start_logging() {
    let log_name = format!("tyls.log");
    simple_logging::log_to_file(log_name, log::LevelFilter::Debug).unwrap();
}

fn notification<N: lsp_types::notification::Notification>(
    params: N::Params,
) -> Message {
    Message::Notification(lsp_server::Notification::new(
        N::METHOD.to_string(),
        params,
    ))
}

fn notify<N: lsp_types::notification::Notification>(
    sender: &Sender<Message>,
    params: N::Params,
) {
    sender.send(notification::<N>(params)).unwrap();
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
            options.legend.token_types = semantic_tokens::legend().types();
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

    let node_at_cursor =
        cst::syntax::traverse::iterate(info.mod_.as_node_or_token(), |node| {
            for child in node.children_with_tokens() {
                if child.range().contains(&offset) {
                    return Step::Continue(child.clone());
                }
            }
            Step::Terminate(node)
        });

    let repr = |node: &cst::syntax::NodeOrToken| -> String {
        format!("{}: {:?}", node.index(), node.kind())
    };

    let mut reprs = vec![repr(&node_at_cursor)];
    for ancestor in node_at_cursor.ancestors() {
        reprs.push(repr(&ancestor.as_node_or_token()));
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
    mod_: cst::syntax::Node,
    errs: Vec<parser::Error>,
    text: String,
    lines_to_offsets: Provider<HashMap<u32, u32>>,
}

impl ModuleInfo {
    fn new(
        mod_: cst::syntax::Node,
        errs: Vec<parser::Error>,
        text: String,
    ) -> Self {
        Self {
            mod_,
            errs,
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
                dispatch_msg(msg, &mut modules, &message_queue);
            }
        });
        sender_thread.join().unwrap();
        receiver_thread.join().unwrap();
    })
    .unwrap();
}

fn dispatch_msg(
    msg: Message,
    modules: &mut HashMap<String, ModuleInfo>,
    message_queue: &ArrayQueue<Message>,
) {
    log::debug!("{msg:?}");
    match msg {
        Message::Notification(not) => match not.method.as_str() {
            "textDocument/didOpen" => {
                let params: lsp_types::DidOpenTextDocumentParams =
                    Deserialize::deserialize(not.params).unwrap();
                let uri = params.text_document.uri;
                let text = &params.text_document.text;
                handle_open_or_change(text, uri, message_queue, modules);
            }
            "textDocument/didChange" => {
                let params: lsp_types::DidChangeTextDocumentParams =
                    Deserialize::deserialize(not.params).unwrap();
                let uri = params.text_document.uri;
                let text = &params.content_changes[0].text;
                handle_open_or_change(text, uri, message_queue, modules);
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
                    let result = find_syntax_tree_at_position(&pos, module)
                        .map(|kind| {
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
                    let tokens = semantic_tokens::compute_from_module(&module);
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

fn handle_open_or_change(
    text: &str,
    uri: lsp_types::Url,
    message_queue: &ArrayQueue<Message>,
    modules: &mut HashMap<String, ModuleInfo>,
) {
    match std::panic::catch_unwind(|| parse_module(text)) {
        Ok(mod_) => {
            let diagnostics = diagnostics_from_mod(&mod_);
            message_queue
                .push(
                    notification::<lsp_types::notification::PublishDiagnostics>(
                        lsp_types::PublishDiagnosticsParams {
                            uri: uri.clone(),
                            version: None,
                            diagnostics,
                        },
                    ),
                )
                .unwrap();
            modules.insert(uri.path().to_string(), mod_);
        }
        Err(e) => {
            let message = if let Some(s) = e.downcast_ref::<&str>() {
                format!("Parser crashed: {s:?}")
            } else {
                format!("Parser crashed!")
            };
            message_queue
                .push(notification::<lsp_types::notification::ShowMessage>(
                    lsp_types::ShowMessageParams {
                        typ: lsp_types::MessageType::ERROR,
                        message,
                    },
                ))
                .unwrap();
        }
    }
}

fn diagnostics_from_mod(mod_: &ModuleInfo) -> Vec<lsp_types::Diagnostic> {
    use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
    mod_.errs
        .iter()
        .map(|err| {
            let start = Position::new(err.pos.line - 1, err.pos.column - 1);
            let end =
                Position::new(start.line, start.character + err.len as u32);
            Diagnostic {
                range: Range::new(start, end),
                message: err.msg.clone(),
                severity: Some(DiagnosticSeverity::ERROR),
                ..Diagnostic::default()
            }
        })
        .collect()
}

fn parse_module(text: &str) -> ModuleInfo {
    let Output { root, errors } = parser::parse_str(text);
    ModuleInfo::new(root, errors, text.to_string())
}
