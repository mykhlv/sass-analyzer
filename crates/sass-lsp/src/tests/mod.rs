use super::*;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tower_lsp_server::LspService;

use std::path::Path;

use crate::config;
use crate::worker::run_worker;
use crate::workspace;

mod basics;
mod call_hierarchy;
mod code_actions;
mod completion;
mod config_commands;
mod config_roots;
mod diagnostics;
mod document_links;
mod empty_file;
mod folding_ranges;
mod goto_definition;
mod highlights;
mod hover;
mod indented_sass;
mod inlay_hints;
mod references_rename;
mod sassdoc;
mod selection_ranges;
mod signature_help;
mod workspace_symbols;

/// Convert a filesystem path to a proper `file://` URI string.
/// Uses `Uri::from_file_path` to match the server's internal URI construction,
/// ensuring URI equality checks work across platforms (especially Windows).
fn file_uri(path: &Path) -> String {
    tower_lsp_server::ls_types::Uri::from_file_path(path)
        .expect("failed to convert path to URI")
        .to_string()
}

/// `languageId` for a `didOpen`, matching the URI's extension. The server picks
/// the parser from the extension, so this only keeps the request realistic.
fn language_id(uri: &str) -> &'static str {
    let uri: tower_lsp_server::ls_types::Uri = uri.parse().expect("valid test URI");
    if crate::worker::is_sass_file(&uri) {
        "sass"
    } else {
        "scss"
    }
}

/// Send a JSON-RPC message with Content-Length framing.
async fn send_msg(writer: &mut (impl AsyncWriteExt + Unpin), msg: &Value) {
    let body = serde_json::to_string(msg).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await.unwrap();
    writer.write_all(body.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
}

/// Read one raw JSON-RPC message from the stream.
async fn recv_msg_raw(reader: &mut (impl AsyncReadExt + Unpin)) -> Value {
    let mut header_buf = Vec::new();
    // Read until we find \r\n\r\n
    loop {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).await.unwrap();
        header_buf.push(byte[0]);
        if header_buf.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header = String::from_utf8(header_buf).unwrap();
    let len: usize = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// Read the next client-facing message, auto-responding to any
/// server→client requests (e.g. `workspace/semanticTokens/refresh`).
async fn recv_msg(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
) -> Value {
    loop {
        let msg = recv_msg_raw(reader).await;
        // Server→client request: has both "id" and "method"
        if msg.get("method").is_some() && msg.get("id").is_some() {
            // Auto-respond with null result
            send_msg(
                writer,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": msg["id"],
                    "result": null
                }),
            )
            .await;
            continue;
        }
        return msg;
    }
}

/// Read until a response with the given `id` arrives, skipping notifications
/// and auto-responding to server→client requests. Use this instead of `recv_msg`
/// when the server may emit extra diagnostics between request and response.
async fn recv_response(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    id: u64,
) -> Value {
    loop {
        let msg = recv_msg_raw(reader).await;
        // Server→client request: auto-respond
        if msg.get("method").is_some() && msg.get("id").is_some() {
            send_msg(
                writer,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": msg["id"],
                    "result": null
                }),
            )
            .await;
            continue;
        }
        // Notification (no "id" field) — skip
        if msg.get("id").is_none() {
            continue;
        }
        // Response with matching id
        if msg["id"] == id {
            return msg;
        }
        // Response with different id — skip (shouldn't happen normally)
    }
}

/// Spawn the LSP server on in-memory duplex streams, return client-side handles.
fn spawn_server() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
    let (client_read, server_write) = tokio::io::duplex(1024 * 64);
    let (server_read, client_write) = tokio::io::duplex(1024 * 64);

    let (service, socket) = LspService::new(|client| {
        let documents = Arc::new(DashMap::new());
        let runtime_config = Arc::new(config::RuntimeConfig::default());
        let module_graph = Arc::new(workspace::ModuleGraph::new(Arc::clone(&runtime_config)));
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_worker(
            task_rx,
            client.clone(),
            Arc::clone(&documents),
            Arc::clone(&module_graph),
            Arc::clone(&runtime_config),
        ));
        Backend {
            client,
            documents,
            source_texts: Arc::new(DashMap::new()),
            module_graph,
            runtime_config,
            task_tx,
            workspace_root: RwLock::new(None),
            workspace_folders: RwLock::new(Vec::new()),
        }
    });
    tokio::spawn(Server::new(server_read, server_write, socket).serve(service));

    (client_read, client_write)
}

async fn do_initialize(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
) -> Value {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "capabilities": {}, "rootUri": null }
        }),
    )
    .await;
    let resp = recv_msg(reader, writer).await;

    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    )
    .await;

    resp
}

async fn do_initialize_with_root(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    root_uri: &str,
) -> Value {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "capabilities": {}, "rootUri": root_uri }
        }),
    )
    .await;
    let resp = recv_msg(reader, writer).await;

    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    )
    .await;

    resp
}

async fn do_initialize_with(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    init_options: Value,
) -> Value {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "capabilities": {},
                "rootUri": "file:///project",
                "initializationOptions": init_options
            }
        }),
    )
    .await;
    let resp = recv_msg(reader, writer).await;

    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    )
    .await;

    resp
}

async fn open_document(writer: &mut (impl AsyncWriteExt + Unpin), uri: &str, text: &str) {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id(uri),
                    "version": 1,
                    "text": text
                }
            }
        }),
    )
    .await;
}

/// Apply LSP `TextEdit`s (as JSON) to `source`. The inputs are ASCII, so a
/// UTF-16 column is a byte column. A position past the last line clamps to EOF,
/// as editors do.
fn apply_lsp_edits(source: &str, edits: &[Value]) -> String {
    let offset = |pos: &Value| {
        let line = usize::try_from(pos["line"].as_u64().unwrap()).unwrap();
        let character = usize::try_from(pos["character"].as_u64().unwrap()).unwrap();
        let line_start: usize = source.split_inclusive('\n').take(line).map(str::len).sum();
        (line_start + character).min(source.len())
    };
    let mut ranges: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|e| {
            (
                offset(&e["range"]["start"]),
                offset(&e["range"]["end"]),
                e["newText"].as_str().unwrap(),
            )
        })
        .collect();
    ranges.sort_by_key(|r| std::cmp::Reverse(r.0));
    let mut out = source.to_owned();
    for (start, end, text) in ranges {
        out.replace_range(start..end, text);
    }
    out
}

/// Assert that `source` parses as indented Sass with no errors.
fn assert_parses_as_sass(source: &str) {
    let (_, errors) = sass_parser::parse_sass(source);
    assert!(errors.is_empty(), "parse errors in:\n{source}\n{errors:?}");
}
