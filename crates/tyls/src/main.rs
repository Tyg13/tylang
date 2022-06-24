use std::collections::HashMap;

use crossbeam_channel::Sender;
use crossbeam_queue::ArrayQueue;
use lsp_server::Connection;
use lsp_server::Message;
use lsp_types::ServerCapabilities;
use serde::de::Deserialize;

fn start_logging() {
    let log_name = format!("tyls.log");
    simple_logging::log_to_file(log_name, log::LevelFilter::Debug).unwrap();
}

fn notify<N: lsp_types::notification::Notification>(sender: &Sender<Message>, params: N::Params) {
    sender
        .send(Message::Notification(lsp_server::Notification::new(
            N::METHOD.to_string(),
            params,
        )))
        .unwrap();
}

fn parse_module(path: &str, text: String) -> ast::Ast<ast::Module> {
    let source = utils::SourceBuilder::new().source(text).file(path).build();
    let tokens = lexer::lex(&source);
    let mut out = Vec::new();
    ast::parse(&source, tokens, &mut out).unwrap()
}

fn server_caps() -> ServerCapabilities {
    let mut server_caps = ServerCapabilities::default();
    server_caps.text_document_sync = Some(lsp_types::TextDocumentSyncCapability::Options({
        let mut options = lsp_types::TextDocumentSyncOptions::default();
        options.open_close = Some(true);
        options.change = Some(lsp_types::TextDocumentSyncKind::FULL);
        options
    }));
    server_caps.hover_provider = Some(lsp_types::HoverProviderCapability::Simple(true));
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

fn find_node_at_position(
    pos: &lsp_types::Position,
    module: &ast::Ast<ast::Module>,
) -> Option<usize> {
    log::debug!("trying to find node at {pos:?}");
    fn contains_pos(span: &utils::Span, pos: &lsp_types::Position) -> bool {
        let (line, column) = ((pos.line + 1) as usize, (pos.character + 1) as usize);
        log::debug!("considering span {span:?}");
        if span.start.line == span.end.line {
            line == span.start.line && span.start.column <= column && column < span.end.column
        } else {
            span.start.line <= line && line < span.end.line
        }
    }
    let candidates: Vec<_> = module
        .metadata
        .spans
        .iter()
        .enumerate()
        .filter_map(|(id, span)| {
            span.and_then(|span| {
                if contains_pos(&span, &pos) {
                    Some((id, span))
                } else {
                    None
                }
            })
        })
        .collect();
    log::debug!("candidates were: {candidates:?}");
    candidates
        .iter()
        .min_by_key(|(_, span)| span.size())
        .map(|(id, _)| id)
        .copied()
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
            let mut modules: HashMap<String, ast::Ast<ast::Module>> = HashMap::new();
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
                            modules.insert(path.clone(), parse_module(&path, text));
                        }
                        "textDocument/didChange" => {
                            let params: lsp_types::DidChangeTextDocumentParams =
                                Deserialize::deserialize(not.params).unwrap();
                            let path = params.text_document.uri.path().to_string();
                            let text = params.content_changes[0].text.to_string();
                            modules.insert(path.clone(), parse_module(&path, text));
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
                            if let Some(module) = modules.get(path) {
                                let pos = params.text_document_position_params.position;
                                let result = find_node_at_position(&pos, &module).map(|node| {
                                    serde_json::to_value(lsp_types::Hover {
                                        contents: lsp_types::HoverContents::Markup(
                                            lsp_types::MarkupContent {
                                                kind: lsp_types::MarkupKind::PlainText,
                                                value: module.kind(node).to_string(),
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
