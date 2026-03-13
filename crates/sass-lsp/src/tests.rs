use super::*;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tower_lsp_server::LspService;
use tower_lsp_server::ls_types::{ParameterLabel, Position, Range, TextDocumentContentChangeEvent};

use crate::completion::fuzzy_score;
use crate::config;
use crate::convert::{apply_content_changes, byte_to_lsp_pos, lsp_pos_to_byte};
use crate::semantic_tokens::{MOD_DECLARATION, TOK_VARIABLE};
use crate::signature_help::parse_param_labels;
use crate::worker::run_worker;
use crate::workspace;

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

#[tokio::test]
async fn initialize_returns_capabilities() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["textDocumentSync"], 2);

    let legend = &caps["semanticTokensProvider"]["legend"];
    let types: Vec<&str> = legend["tokenTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            "variable",
            "function",
            "mixin",
            "parameter",
            "property",
            "type"
        ]
    );

    let info = &resp["result"]["serverInfo"];
    assert_eq!(info["name"], "sass-analyzer");
}

#[tokio::test]
async fn did_open_publishes_diagnostics() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///test.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": ".btn { color: red; }"
                }
            }
        }),
    )
    .await;

    // Worker debounce fires, publishes diagnostics
    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "valid SCSS should have 0 diagnostics");
}

#[tokio::test]
async fn did_open_with_errors_publishes_diagnostics() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///bad.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": "{{ invalid"
                }
            }
        }),
    )
    .await;

    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert!(!diags.is_empty(), "invalid SCSS should have diagnostics");
}

#[tokio::test]
async fn semantic_tokens_for_scss() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.btn { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///tokens.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;

    // Wait for diagnostics (means parse is done)
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/semanticTokens/full",
            "params": {
                "textDocument": { "uri": "file:///tokens.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let data = resp["result"]["data"].as_array().unwrap();
    // Each token is 5 u32s; expect: $color(decl), color(prop), $color(ref)
    assert_eq!(data.len() % 5, 0, "data length must be multiple of 5");
    let token_count = data.len() / 5;
    assert_eq!(
        token_count, 3,
        "expected 3 tokens: $color decl, color prop, $color ref"
    );

    // First token: $color declaration at line 0, col 0
    assert_eq!(data[0], 0, "delta_line");
    assert_eq!(data[1], 0, "delta_start");
    assert_eq!(data[2], 6, "length of $color");
    assert_eq!(data[3], TOK_VARIABLE, "token_type = VARIABLE");
    assert_eq!(data[4], MOD_DECLARATION, "modifier = DECLARATION");
}

#[tokio::test]
async fn did_close_clears_diagnostics() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///close.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": ".a { color: red; }"
                }
            }
        }),
    )
    .await;
    let _ = recv_msg(&mut reader, &mut writer).await; // open diagnostics

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": { "uri": "file:///close.scss" }
            }
        }),
    )
    .await;

    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "close should clear diagnostics");
}

#[tokio::test(start_paused = true)]
async fn debounce_coalesces_rapid_changes() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Open document
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///debounce.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": ".a {}"
                }
            }
        }),
    )
    .await;

    // Advance past debounce to get initial diagnostics
    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;
    let _ = recv_msg(&mut reader, &mut writer).await;

    // Send 5 rapid changes within 20ms (well under 50ms debounce)
    for v in 2..=6 {
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": "file:///debounce.scss", "version": v },
                    "contentChanges": [{ "text": format!(".v{v} {{}}") }]
                }
            }),
        )
        .await;
        tokio::time::advance(Duration::from_millis(4)).await;
        tokio::task::yield_now().await;
    }

    // Advance past debounce deadline
    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;

    // Should get exactly one diagnostics notification (coalesced)
    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    // Version should be the latest (6)
    assert_eq!(notif["params"]["version"], 6);
}

#[tokio::test]
async fn document_symbols_for_scss() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$primary: blue;\n@mixin btn($size) { font-size: $size; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///symbols.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;

    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": "file:///symbols.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 2, "expected 2 symbols: $primary and btn");

    assert_eq!(result[0]["name"], "primary");
    assert_eq!(result[0]["kind"], 13); // SymbolKind::VARIABLE = 13

    assert_eq!(result[1]["name"], "btn");
    assert_eq!(result[1]["kind"], 12); // SymbolKind::FUNCTION = 12
    assert!(result[1]["detail"].as_str().unwrap().contains("@mixin"));
}

#[tokio::test]
async fn goto_definition_variable() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.btn { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///def.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on $color reference at line 1, character 15
    // Line 1: ".btn { color: $color; }"
    //                        ^ char 14 ($), char 15 (c)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": "file:///def.scss" },
                "position": { "line": 1, "character": 15 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert_eq!(result["uri"], "file:///def.scss");
    // Definition: $color at line 0, characters 0..6
    assert_eq!(result["range"]["start"]["line"], 0);
    assert_eq!(result["range"]["start"]["character"], 0);
    assert_eq!(result["range"]["end"]["line"], 0);
    assert_eq!(result["range"]["end"]["character"], 6);
}

#[tokio::test]
async fn goto_definition_mixin() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin btn { display: block; }\n.card { @include btn; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///mixin.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "btn" in @include btn at line 1
    // Line 1: ".card { @include btn; }"
    //                          ^ char 17
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": "file:///mixin.scss" },
                "position": { "line": 1, "character": 17 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert_eq!(result["uri"], "file:///mixin.scss");
    // @mixin btn → name "btn" starts at character 7
    assert_eq!(result["range"]["start"]["line"], 0);
    assert_eq!(result["range"]["start"]["character"], 7);
}

#[tokio::test]
async fn goto_definition_function() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function double($n) { @return $n * 2; }\n.x { width: double(5px); }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///func.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "double" call at line 1
    // Line 1: ".x { width: double(5px); }"
    //                      ^ char 12
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": "file:///func.scss" },
                "position": { "line": 1, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert_eq!(result["uri"], "file:///func.scss");
    // @function double → name "double" at character 10
    assert_eq!(result["range"]["start"]["line"], 0);
    assert_eq!(result["range"]["start"]["character"], 10);
}

#[tokio::test]
async fn goto_definition_returns_null_on_definition() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///nodef.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on $color declaration itself (should return null)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": "file:///nodef.scss" },
                "position": { "line": 0, "character": 1 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["result"].is_null(),
        "definition on a decl should be null"
    );
}

#[tokio::test]
async fn completion_returns_local_symbols() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n@mixin btn { }\n@function double($n) { @return $n * 2; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp.scss" },
                "position": { "line": 2, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(
        items.len(),
        3,
        "expected 3 completions: $color, btn, double"
    );

    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"$color"), "should contain $color");
    assert!(labels.contains(&"btn"), "should contain btn (mixin)");
    assert!(
        labels.contains(&"double"),
        "should contain double (function)"
    );
}

#[tokio::test]
async fn completion_returns_none_for_unknown_uri() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///unknown.scss" },
                "position": { "line": 0, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["result"].is_null(),
        "completion for unknown file should be null"
    );
}

#[tokio::test]
async fn completion_item_kinds() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$v: 1;\n@mixin m { }\n@function f() { @return 1; }\n%ph { }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///kinds.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///kinds.scss" },
                "position": { "line": 3, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 4, "expected 4 completions");

    let var = items.iter().find(|i| i["label"] == "$v").unwrap();
    assert_eq!(var["kind"], 6, "variable kind = 6");

    let mixin = items.iter().find(|i| i["label"] == "m").unwrap();
    assert_eq!(mixin["kind"], 2, "mixin kind = METHOD = 2");
    assert!(
        mixin["detail"].as_str().unwrap().contains("@mixin"),
        "mixin detail should contain @mixin"
    );

    let func = items.iter().find(|i| i["label"] == "f").unwrap();
    assert_eq!(func["kind"], 3, "function kind = 3");

    let placeholder = items.iter().find(|i| i["label"] == "%ph").unwrap();
    assert_eq!(placeholder["kind"], 7, "placeholder kind = CLASS = 7");
}

#[tokio::test]
async fn completion_after_dollar_only_variables() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss =
        "$color: red;\n@mixin btn { }\n@function double($n) { @return $n * 2; }\n.a { color: $";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_dollar.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "$" on line 3
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_dollar.scss" },
                "position": { "line": 3, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    // Should only contain variables, not mixins or functions
    for item in items {
        assert_eq!(
            item["kind"], 6,
            "after $ only variable items (kind=6), got: {}",
            item["label"]
        );
    }
    assert!(
        items.iter().any(|i| i["label"] == "$color"),
        "should contain $color"
    );
}

#[tokio::test]
async fn completion_after_include_only_mixins() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n@mixin btn { }\n@function double($n) { @return $n * 2; }\n.a {\n  @include \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_include.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on line 4: "  @include " (char 11 = end of "@include ")
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_include.scss" },
                "position": { "line": 4, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 1, "only mixins after @include");
    assert_eq!(items[0]["label"], "btn");
    assert_eq!(items[0]["kind"], 2, "mixin kind = METHOD = 2");
}

#[tokio::test]
async fn completion_sort_text_tiers() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$local: 1;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_sort.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_sort.scss" },
                "position": { "line": 0, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    // Local symbols should have sortText starting with "0_"
    let local = items.iter().find(|i| i["label"] == "$local").unwrap();
    assert!(
        local["sortText"].as_str().unwrap().starts_with("0_"),
        "local symbol sortText should start with 0_"
    );
}

#[tokio::test]
async fn completion_property_name_context() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  col\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_prop.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "col" at line 1, character 5
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 33,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_prop.scss" },
                "position": { "line": 1, "character": 5 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert!(!items.is_empty(), "should have CSS property completions");
    // All items should have kind = PROPERTY (10)
    for item in items {
        assert_eq!(item["kind"], 10, "property completion kind should be 10");
    }
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"color"), "should contain 'color'");
    assert!(
        labels.contains(&"column-count"),
        "should contain 'column-count'"
    );
}

#[tokio::test]
async fn completion_use_path_builtins() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@use \"sass:";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_use.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 34,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_use.scss" },
                "position": { "line": 0, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"sass:math"), "should contain sass:math");
    assert!(labels.contains(&"sass:color"), "should contain sass:color");
    assert!(labels.contains(&"sass:list"), "should contain sass:list");
}

#[tokio::test]
async fn completion_variable_shows_value_detail() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$primary: #3498db;\n.a { color: $";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_detail.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 35,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_detail.scss" },
                "position": { "line": 1, "character": 14 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let primary = items.iter().find(|i| i["label"] == "$primary").unwrap();
    assert_eq!(
        primary["detail"].as_str().unwrap(),
        "#3498db",
        "variable detail should show its value"
    );
}

// ── CSS value completion tests ─────────────────────────────────────

#[tokio::test]
async fn completion_property_value_display() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  display: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_display.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 36,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_display.scss" },
                "position": { "line": 1, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"flex"), "should contain 'flex'");
    assert!(labels.contains(&"grid"), "should contain 'grid'");
    assert!(labels.contains(&"block"), "should contain 'block'");
    assert!(labels.contains(&"none"), "should contain 'none'");
    assert!(
        labels.contains(&"inline-flex"),
        "should contain 'inline-flex'"
    );
    // Negative: position values should NOT appear in display completions
    assert!(
        !labels.contains(&"absolute"),
        "display should not contain 'absolute'"
    );
    assert!(
        !labels.contains(&"sticky"),
        "display should not contain 'sticky'"
    );
}

#[tokio::test]
async fn completion_property_value_position() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  position: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_pos.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 37,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_pos.scss" },
                "position": { "line": 1, "character": 12 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"absolute"), "should contain 'absolute'");
    assert!(labels.contains(&"relative"), "should contain 'relative'");
    assert!(labels.contains(&"fixed"), "should contain 'fixed'");
    assert!(labels.contains(&"sticky"), "should contain 'sticky'");
    assert!(labels.contains(&"static"), "should contain 'static'");
    // Negative: display values should NOT appear in position completions
    assert!(
        !labels.contains(&"flex"),
        "position should not contain 'flex'"
    );
    assert!(
        !labels.contains(&"grid"),
        "position should not contain 'grid'"
    );
}

#[tokio::test]
async fn completion_property_value_with_partial() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  display: fl\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_partial.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 38,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_partial.scss" },
                "position": { "line": 1, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"flex"), "should contain 'flex'");
    assert!(labels.contains(&"flow-root"), "should contain 'flow-root'");
    // "flex" should rank before "flow-root" (prefix match wins)
    let flex_idx = labels.iter().position(|l| *l == "flex").unwrap();
    let flow_idx = labels.iter().position(|l| *l == "flow-root").unwrap();
    assert!(flex_idx < flow_idx, "'flex' should rank before 'flow-root'");
}

#[tokio::test]
async fn completion_property_value_includes_variables() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$my-display: flex;\n.a {\n  display: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_vars.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 39,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_vars.scss" },
                "position": { "line": 2, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    // Should have both CSS keyword values and Sass variables
    assert!(labels.contains(&"flex"), "should contain keyword 'flex'");
    assert!(
        labels.contains(&"$my-display"),
        "should contain variable '$my-display'"
    );
}

#[tokio::test]
async fn completion_property_value_unknown_property() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$x: 1;\n.a {\n  custom-prop: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_unknown.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 40,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_unknown.scss" },
                "position": { "line": 2, "character": 15 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    // Unknown property still gets global keywords + Sass symbols
    assert!(labels.contains(&"inherit"), "should contain 'inherit'");
    assert!(labels.contains(&"initial"), "should contain 'initial'");
    assert!(labels.contains(&"$x"), "should contain variable '$x'");
}

#[tokio::test]
async fn completion_property_value_global_keywords() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  display: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_global.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 41,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_global.scss" },
                "position": { "line": 1, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"inherit"), "should contain 'inherit'");
    assert!(labels.contains(&"initial"), "should contain 'initial'");
    assert!(labels.contains(&"unset"), "should contain 'unset'");
    assert!(labels.contains(&"revert"), "should contain 'revert'");
    assert!(
        labels.contains(&"revert-layer"),
        "should contain 'revert-layer'"
    );
}

#[tokio::test]
async fn completion_property_value_flex_direction() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  flex-direction: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_flexdir.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_flexdir.scss" },
                "position": { "line": 1, "character": 18 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"row"), "should contain 'row'");
    assert!(labels.contains(&"column"), "should contain 'column'");
    assert!(
        labels.contains(&"row-reverse"),
        "should contain 'row-reverse'"
    );
    assert!(
        labels.contains(&"column-reverse"),
        "should contain 'column-reverse'"
    );
}

#[tokio::test]
async fn completion_property_value_enum_member_kind() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  position: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_kind.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 43,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_kind.scss" },
                "position": { "line": 1, "character": 12 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    // CSS value keywords should have kind = ENUM_MEMBER (20)
    let keyword_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| {
            let label = i["label"].as_str().unwrap_or("");
            !label.starts_with('$')
        })
        .collect();
    assert!(!keyword_items.is_empty());
    for item in keyword_items {
        assert_eq!(
            item["kind"], 20,
            "CSS value keyword should have kind ENUM_MEMBER (20), got {:?} for {:?}",
            item["kind"], item["label"]
        );
    }
}

#[tokio::test]
async fn completion_map_entry_not_property_value() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Map entry on its own line — should NOT get CSS value completions
    let scss = "$map: (\n  key: \n);\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///map_entry.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "key: " on line 1
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 60,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///map_entry.scss" },
                "position": { "line": 1, "character": 7 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    // Should NOT contain CSS value keywords like "flex", "grid", "none"
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(
            !labels.contains(&"flex"),
            "map entry should not offer CSS value 'flex', got: {labels:?}"
        );
        assert!(
            !labels.contains(&"grid"),
            "map entry should not offer CSS value 'grid', got: {labels:?}"
        );
    }
}

#[tokio::test]
async fn completion_multiline_value_context() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Multi-line declaration: value on a continuation line
    let scss = ".a {\n  display:\n    \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///multiline_val.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on line 2 (the continuation line after "display:\n")
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 61,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///multiline_val.scss" },
                "position": { "line": 2, "character": 4 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // Should offer display values since we're in value position
    assert!(
        labels.contains(&"flex"),
        "multi-line display value should offer 'flex', got: {labels:?}"
    );
    assert!(
        labels.contains(&"grid"),
        "multi-line display value should offer 'grid', got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_map_entry_with_css_property_key() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Map keys that happen to be valid CSS property names
    let scss = "$map: (\n  display: flex,\n  position: \n);\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///map_css_key.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "position: " on line 2 — inside a map, NOT a CSS declaration
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 62,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///map_css_key.scss" },
                "position": { "line": 2, "character": 12 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        // Should NOT offer CSS position values like "absolute", "fixed", "sticky"
        assert!(
            !labels.contains(&"absolute"),
            "map entry should not offer CSS value 'absolute', got: {labels:?}"
        );
        assert!(
            !labels.contains(&"sticky"),
            "map entry should not offer CSS value 'sticky', got: {labels:?}"
        );
    }
}

#[tokio::test]
async fn completion_multiline_value_with_partial() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Multi-line declaration with partial value text on continuation line
    let scss = ".a {\n  display:\n    fl\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///multiline_partial.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "fl" on line 2
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 63,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///multiline_partial.scss" },
                "position": { "line": 2, "character": 6 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions for multi-line partial");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // "flex" should match the "fl" prefix and be offered
    assert!(
        labels.contains(&"flex"),
        "multi-line partial 'fl' should offer 'flex', got: {labels:?}"
    );
    assert!(
        labels.contains(&"flow-root"),
        "multi-line partial 'fl' should offer 'flow-root', got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_multiline_value_multiple_declarations() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Multiple declarations; cursor on continuation line after the second one
    let scss = ".a {\n  color: red;\n  display:\n    \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///multi_decl.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on the blank continuation line (line 3) after "display:"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 64,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///multi_decl.scss" },
                "position": { "line": 3, "character": 4 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions for display after color");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // Should offer display values, not color values
    assert!(
        labels.contains(&"flex"),
        "should offer display values like 'flex', got: {labels:?}"
    );
    assert!(
        labels.contains(&"grid"),
        "should offer display values like 'grid', got: {labels:?}"
    );
    // Should NOT offer color-specific values (red is not a CSS keyword we enumerate)
    assert!(
        !labels.contains(&"absolute"),
        "should not offer position values, got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_nested_map_not_property_value() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Deeply nested map — inner key should not trigger CSS value completions
    let scss = "$theme: (\n  colors: (\n    primary: \n  )\n);\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///nested_map.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "primary: " on line 2
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 65,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///nested_map.scss" },
                "position": { "line": 2, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(
            !labels.contains(&"flex"),
            "nested map entry should not offer CSS value 'flex', got: {labels:?}"
        );
        assert!(
            !labels.contains(&"inherit"),
            "nested map entry should not offer global keyword 'inherit', got: {labels:?}"
        );
    }
}

#[tokio::test]
async fn completion_custom_property_multiline() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Custom property with value on continuation line
    let scss = ".a {\n  --my-display:\n    \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///custom_prop_ml.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on blank continuation line (line 2) after "--my-display:"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 66,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///custom_prop_ml.scss" },
                "position": { "line": 2, "character": 4 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions for custom property value");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // Custom properties accept any value; we should at least get global keywords
    assert!(
        labels.contains(&"inherit"),
        "custom property value should offer 'inherit', got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_decimal_not_namespace() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  font-size: 1.\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///decimal.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "1." — should NOT treat "1" as namespace
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 67,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///decimal.scss" },
                "position": { "line": 1, "character": 15 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    // Should get PropertyValue completions (font-size keywords or global keywords),
    // not an empty namespace result
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        // Should NOT be empty (which would happen if "1" was treated as a namespace)
        // PropertyValue for font-size doesn't have keyword values, but global keywords apply
        assert!(
            !labels.is_empty(),
            "decimal position should still offer completions, got empty"
        );
    }
}

#[tokio::test]
async fn completion_include_namespace_prefix() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // File with @use that creates a namespace, then @include with that namespace
    let scss = "@use \"sass:math\";\n.a {\n  @include math.\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///include_ns.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "math." on line 2
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 68,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///include_ns.scss" },
                "position": { "line": 2, "character": 17 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    // With Namespace context, only symbols from math namespace should appear
    if let Some(items) = resp["result"].as_array() {
        for item in items {
            let label = item["label"].as_str().unwrap_or("");
            assert!(
                label.starts_with("math."),
                "all items should be from math namespace, got: {label}"
            );
        }
    }
}

#[tokio::test]
async fn hover_on_variable_reference() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.btn { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover.scss" },
                "position": { "line": 1, "character": 15 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("$color"),
        "hover should show variable name"
    );
    assert!(content.contains("red"), "hover should show value");
}

#[tokio::test]
async fn hover_on_variable_definition() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$primary: blue;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_def.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Hover on the $ of $primary definition
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_def.scss" },
                "position": { "line": 0, "character": 1 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("$primary"),
        "hover on def should show name"
    );
    assert!(content.contains("blue"), "hover on def should show value");
}

#[tokio::test]
async fn hover_on_mixin_reference() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin btn($size) { font-size: $size; }\n.card { @include btn(16px); }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_mixin.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Hover on "btn" in @include btn(16px)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_mixin.scss" },
                "position": { "line": 1, "character": 17 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(content.contains("@mixin"), "hover should show @mixin");
    assert!(content.contains("btn"), "hover should show mixin name");
}

#[tokio::test]
async fn hover_returns_null_on_empty_space() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n\n.btn { }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_null.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Hover on empty line
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 33,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_null.scss" },
                "position": { "line": 1, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["result"].is_null(),
        "hover on empty space should be null"
    );
}

#[tokio::test]
async fn hover_with_doc_comment() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "/// The primary color\n$primary: #333;\n.a { color: $primary; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_doc.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 34,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_doc.scss" },
                "position": { "line": 2, "character": 14 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("primary color"),
        "hover should show doc comment"
    );
}

#[tokio::test]
async fn hover_builtin_function_shows_doc_url() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@use \"sass:math\";\n.a { width: math.ceil(1.5); }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_builtin.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Hover on "ceil" (line 1, character 17 = inside "ceil")
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 35,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_builtin.scss" },
                "position": { "line": 1, "character": 17 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("@function ceil"),
        "hover should show function signature"
    );
    assert!(
        content.contains("sass-lang.com/documentation/modules/math/#ceil"),
        "hover should contain doc URL: {content}"
    );
}

#[tokio::test]
async fn hover_builtin_variable_shows_doc_url() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@use \"sass:math\";\n.a { content: math.$pi; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_builtin_var.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Hover on "pi" (line 1, character 20 = inside "$pi")
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 36,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_builtin_var.scss" },
                "position": { "line": 1, "character": 20 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(content.contains("$pi"), "hover should show variable name");
    assert!(
        content.contains("sass-lang.com/documentation/modules/math/#%24pi"),
        "hover should contain doc URL with $ anchor: {content}"
    );
}

#[tokio::test]
async fn initialize_reports_hover_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;
    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["hoverProvider"], true);
}

#[tokio::test]
async fn initialize_reports_references_and_rename_capabilities() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;
    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["referencesProvider"], true);
    assert!(caps["renameProvider"].is_object());
}

#[tokio::test]
async fn references_variable_same_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.a { color: $color; }\n.b { border-color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///refs.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // References on $color usage at line 1
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 40,
            "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": "file:///refs.scss" },
                "position": { "line": 1, "character": 15 },
                "context": { "includeDeclaration": false }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let locs = resp["result"].as_array().unwrap();
    // Should find 2 references (not including declaration)
    assert_eq!(locs.len(), 2, "expected 2 references");
}

#[tokio::test]
async fn references_include_declaration() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$x: 1;\n.a { width: $x; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///refs_decl.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 41,
            "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": "file:///refs_decl.scss" },
                "position": { "line": 1, "character": 13 },
                "context": { "includeDeclaration": true }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let locs = resp["result"].as_array().unwrap();
    assert_eq!(locs.len(), 2, "expected 2: declaration + 1 ref");
}

#[tokio::test]
async fn references_mixin() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin btn { }\n.a { @include btn; }\n.b { @include btn; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///refs_mixin.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": "file:///refs_mixin.scss" },
                "position": { "line": 1, "character": 17 },
                "context": { "includeDeclaration": false }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let locs = resp["result"].as_array().unwrap();
    assert_eq!(locs.len(), 2, "expected 2 mixin references");
}

#[tokio::test]
async fn references_returns_null_on_empty() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a { color: red; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///refs_null.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 43,
            "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": "file:///refs_null.scss" },
                "position": { "line": 0, "character": 5 },
                "context": { "includeDeclaration": true }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(resp["result"].is_null());
}

#[tokio::test]
async fn prepare_rename_variable() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.a { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///prep_rename.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 44,
            "method": "textDocument/prepareRename",
            "params": {
                "textDocument": { "uri": "file:///prep_rename.scss" },
                "position": { "line": 1, "character": 15 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert_eq!(result["placeholder"], "color");
}

#[tokio::test]
async fn prepare_rename_on_definition() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///prep_rename_def.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 45,
            "method": "textDocument/prepareRename",
            "params": {
                "textDocument": { "uri": "file:///prep_rename_def.scss" },
                "position": { "line": 0, "character": 1 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert_eq!(result["placeholder"], "color");
}

#[tokio::test]
async fn rename_variable_single_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.a { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///rename.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 46,
            "method": "textDocument/rename",
            "params": {
                "textDocument": { "uri": "file:///rename.scss" },
                "position": { "line": 1, "character": 15 },
                "newName": "primary"
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let changes = &resp["result"]["changes"]["file:///rename.scss"];
    let edits = changes.as_array().unwrap();
    assert!(edits.len() >= 2, "expected at least 2 edits (decl + ref)");
    for edit in edits {
        assert_eq!(edit["newText"], "primary");
    }
}

#[tokio::test]
async fn rename_mixin() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin btn { }\n.a { @include btn; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///rename_mixin.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 47,
            "method": "textDocument/rename",
            "params": {
                "textDocument": { "uri": "file:///rename_mixin.scss" },
                "position": { "line": 1, "character": 17 },
                "newName": "button"
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let changes = &resp["result"]["changes"]["file:///rename_mixin.scss"];
    let edits = changes.as_array().unwrap();
    assert!(edits.len() >= 2, "expected at least 2 edits");
    for edit in edits {
        assert_eq!(edit["newText"], "button");
    }
}

#[tokio::test]
async fn rename_returns_null_on_empty_space() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a { color: red; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///rename_null.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 48,
            "method": "textDocument/rename",
            "params": {
                "textDocument": { "uri": "file:///rename_null.scss" },
                "position": { "line": 0, "character": 5 },
                "newName": "x"
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(resp["result"].is_null());
}

#[tokio::test]
async fn rename_conflict_returns_error() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n$primary: blue;\n.a { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///rename_conflict.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Try to rename $color to $primary — should fail (conflict)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 80,
            "method": "textDocument/rename",
            "params": {
                "textDocument": { "uri": "file:///rename_conflict.scss" },
                "position": { "line": 2, "character": 15 },
                "newName": "primary"
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(resp["error"].is_object(), "expected error response");
    let msg = resp["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("already exists"),
        "error message should mention conflict: {msg}"
    );
}

#[tokio::test]
async fn rename_no_conflict_different_kind() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // $color and @function color — different kinds, rename should succeed
    let scss = "$color: red;\n@function color() { @return red; }\n.a { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///rename_no_conflict.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Rename $color to "shade" — no conflict because "shade" doesn't exist
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 81,
            "method": "textDocument/rename",
            "params": {
                "textDocument": { "uri": "file:///rename_no_conflict.scss" },
                "position": { "line": 2, "character": 15 },
                "newName": "shade"
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["error"].is_null(),
        "should succeed: no conflict with different kind"
    );
    let changes = &resp["result"]["changes"]["file:///rename_no_conflict.scss"];
    let edits = changes.as_array().unwrap();
    assert!(edits.len() >= 2, "expected at least 2 edits (decl + ref)");
}

// ── Signature help tests ────────────────────────────────────────

#[tokio::test]
async fn initialize_reports_signature_help_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let sig_help = &resp["result"]["capabilities"]["signatureHelpProvider"];
    assert!(!sig_help.is_null(), "should have signatureHelpProvider");
    let triggers: Vec<&str> = sig_help["triggerCharacters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(triggers, ["(", ","]);
}

#[tokio::test]
async fn signature_help_function_call() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1px, ); }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///sig_func.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "add(1px, " → active param = 1
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 60,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": "file:///sig_func.scss" },
                "position": { "line": 1, "character": 21 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert!(!result.is_null(), "should return signature help");
    let sigs = result["signatures"].as_array().unwrap();
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0]["label"], "@function add($a, $b)");
    assert_eq!(result["activeParameter"], 1);
    let params = sigs[0]["parameters"].as_array().unwrap();
    assert_eq!(params.len(), 2);
}

#[tokio::test]
async fn signature_help_mixin_include() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin btn($size, $color: red) { font-size: $size; }\n.a { @include btn(16px); }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///sig_mixin.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "@include btn(" → active param = 0
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 61,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": "file:///sig_mixin.scss" },
                "position": { "line": 1, "character": 18 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert!(!result.is_null(), "should return signature help for mixin");
    let sigs = result["signatures"].as_array().unwrap();
    assert_eq!(sigs[0]["label"], "@mixin btn($size, $color: red)");
    assert_eq!(result["activeParameter"], 0);
    let params = sigs[0]["parameters"].as_array().unwrap();
    assert_eq!(params.len(), 2);
}

#[tokio::test]
async fn signature_help_active_parameter_tracking() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function f($a, $b, $c) { @return 0; }\n.x { width: f(1, 2, ); }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///sig_active.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "f(1, 2, " → active param = 2
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 62,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": "file:///sig_active.scss" },
                "position": { "line": 1, "character": 20 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(resp["result"]["activeParameter"], 2);
}

#[tokio::test]
async fn signature_help_returns_null_outside_call() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///sig_null.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 63,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": "file:///sig_null.scss" },
                "position": { "line": 0, "character": 5 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["result"].is_null(),
        "should be null outside function call"
    );
}

#[tokio::test]
async fn signature_help_with_doc_comment() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "/// Scales a value\n@function scale($value, $factor: 2) { @return $value * $factor; }\n.x { width: scale(10px); }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///sig_doc.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 64,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": "file:///sig_doc.scss" },
                "position": { "line": 2, "character": 18 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    let sig = &result["signatures"][0];
    assert_eq!(sig["label"], "@function scale($value, $factor: 2)");
    let doc = &sig["documentation"];
    assert_eq!(doc["value"], "Scales a value");
}

#[test]
fn parse_param_labels_offsets() {
    let params = "($a, $b: 1px)";
    let sig = format!("@function f{params}");
    let result = parse_param_labels(&sig, params);
    assert_eq!(result.len(), 2);
    // "$a" starts at offset 12 (after "@function f("), ends at 14
    if let ParameterLabel::LabelOffsets([s, e]) = result[0].label {
        assert_eq!(&sig[s as usize..e as usize], "$a");
    } else {
        panic!("expected LabelOffsets");
    }
    // "$b: 1px" starts at 16 (after "$a, "), ends at 23
    if let ParameterLabel::LabelOffsets([s, e]) = result[1].label {
        assert_eq!(&sig[s as usize..e as usize], "$b: 1px");
    } else {
        panic!("expected LabelOffsets");
    }
}

#[test]
fn parse_param_labels_non_ascii_utf16() {
    // "ціна" is 8 bytes in UTF-8 but 4 UTF-16 code units
    let params = "($ціна, $b)";
    let sig = format!("@mixin m{params}");
    let result = parse_param_labels(&sig, params);
    assert_eq!(result.len(), 2);
    // "$ціна": starts at UTF-16 offset 8 ("@mixin m("), 5 UTF-16 code units long
    if let ParameterLabel::LabelOffsets([s, e]) = result[0].label {
        assert_eq!(s, 9); // "@mixin m(" = 9 UTF-16 units
        assert_eq!(e, 14); // "$ціна" = 5 UTF-16 units ($+ц+і+н+а)
    } else {
        panic!("expected LabelOffsets");
    }
    // "$b": after "$ціна, "
    if let ParameterLabel::LabelOffsets([s, e]) = result[1].label {
        assert_eq!(e - s, 2); // "$b" = 2 UTF-16 units
    } else {
        panic!("expected LabelOffsets");
    }
}

// ── Workspace symbol tests ──────────────────────────────────────

#[tokio::test]
async fn initialize_reports_workspace_symbol_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;
    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["workspaceSymbolProvider"], true);
}

#[tokio::test]
async fn workspace_symbol_returns_matching_symbols() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$primary: blue;\n@mixin btn($size) { }\n@function scale($n) { @return $n; }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Search for "btn"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 70,
            "method": "workspace/symbol",
            "params": { "query": "btn" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "btn");
}

#[tokio::test]
async fn workspace_symbol_empty_query_returns_all() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$a: 1;\n$b: 2;";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws_all.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 71,
            "method": "workspace/symbol",
            "params": { "query": "" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 2, "empty query should return all symbols");
}

#[tokio::test]
async fn workspace_symbol_fuzzy_matching() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin responsive-grid { }\n@mixin simple { }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws_fuzz.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // "rg" should fuzzy-match "responsive-grid" but not "simple"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 72,
            "method": "workspace/symbol",
            "params": { "query": "rg" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "responsive-grid");
}

#[tokio::test]
async fn workspace_symbol_no_match_returns_null() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$x: 1;";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws_none.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 73,
            "method": "workspace/symbol",
            "params": { "query": "zzz" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(resp["result"].is_null(), "no match should return null");
}

#[test]
fn fuzzy_score_basics() {
    // Exact match → highest score.
    assert_eq!(fuzzy_score("color", "color"), Some(1000));
    // Prefix match → 500+.
    assert!(fuzzy_score("color-primary", "color").unwrap() >= 500);
    // Word boundary match → 200+ (r and g match starts of "responsive" and "grid").
    let rg_score = fuzzy_score("responsive-grid", "rg").unwrap();
    assert!(
        rg_score >= 200,
        "word boundary should score 200+, got {rg_score}"
    );
    // Subsequence match → >0.
    assert!(fuzzy_score("primary", "pry").unwrap() > 0);
    // No match → None.
    assert_eq!(fuzzy_score("simple", "rg"), None);
    // Empty query → matches everything.
    assert_eq!(fuzzy_score("anything", ""), Some(0));
}

#[test]
fn fuzzy_score_ranking() {
    let exact = fuzzy_score("color", "color").unwrap();
    let prefix = fuzzy_score("color-primary", "color").unwrap();
    let boundary = fuzzy_score("responsive-grid", "rg").unwrap();
    let subseq = fuzzy_score("primary", "pry").unwrap();
    assert!(exact > prefix, "exact > prefix");
    assert!(prefix > boundary, "prefix > boundary");
    assert!(boundary > subseq, "boundary > subsequence");
}

#[test]
fn fuzzy_score_camel_case_boundary() {
    // camelCase boundary: "bc" matches "B" from "border" and "C" from "Color"
    let score = fuzzy_score("borderColor", "bc").unwrap();
    assert!(score >= 200, "camelCase boundary match, got {score}");
}

#[test]
fn completion_context_detection() {
    use crate::completion::{CompletionContext, detect_completion_context};

    // After `$` → Variable
    let ctx = detect_completion_context("  color: $", 10);
    assert!(matches!(ctx, CompletionContext::Variable));

    // After `@include ` → IncludeMixin
    let ctx = detect_completion_context("  @include ", 11);
    assert!(matches!(ctx, CompletionContext::IncludeMixin));

    // After `@use "` → UseModulePath
    let ctx = detect_completion_context("  @use \"", 8);
    assert!(matches!(ctx, CompletionContext::UseModulePath(_)));

    // On `bor` → PropertyName
    let ctx = detect_completion_context("  bor", 5);
    assert!(matches!(ctx, CompletionContext::PropertyName(_)));

    // After `color:` → PropertyValue with property name and partial
    let ctx = detect_completion_context("  color: ", 9);
    assert!(
        matches!(ctx, CompletionContext::PropertyValue(ref p, ref v) if p == "color" && v.is_empty()),
        "expected PropertyValue(\"color\", \"\"), got {ctx:?}"
    );

    // After `display: fl` → PropertyValue with partial
    let ctx = detect_completion_context("  display: fl", 13);
    assert!(
        matches!(ctx, CompletionContext::PropertyValue(ref p, ref v) if p == "display" && v == "fl"),
        "expected PropertyValue(\"display\", \"fl\"), got {ctx:?}"
    );

    // Pseudo-selectors must NOT be detected as PropertyValue
    let ctx = detect_completion_context("  a:hover", 9);
    assert!(
        !matches!(ctx, CompletionContext::PropertyValue(..)),
        "a:hover should not be PropertyValue, got {ctx:?}"
    );

    let ctx = detect_completion_context("  &:focus", 9);
    assert!(
        !matches!(ctx, CompletionContext::PropertyValue(..)),
        "&:focus should not be PropertyValue, got {ctx:?}"
    );

    let ctx = detect_completion_context("  :root", 7);
    assert!(
        !matches!(ctx, CompletionContext::PropertyValue(..)),
        ":root should not be PropertyValue, got {ctx:?}"
    );

    // Decimal number must NOT trigger Namespace (e.g., `font-size: 1.`)
    let ctx = detect_completion_context("  font-size: 1.", 15);
    assert!(
        !matches!(ctx, CompletionContext::Namespace(_)),
        "decimal 1. should not be Namespace, got {ctx:?}"
    );

    // @include with namespace prefix → Namespace, not IncludeMixin
    let ctx = detect_completion_context("  @include math.", 16);
    assert!(
        matches!(ctx, CompletionContext::Namespace(ref ns) if ns == "math"),
        "expected Namespace(\"math\"), got {ctx:?}"
    );

    // @include without namespace → IncludeMixin
    let ctx = detect_completion_context("  @include btn", 14);
    assert!(matches!(ctx, CompletionContext::IncludeMixin));

    // @extend → Extend
    let ctx = detect_completion_context("  @extend %btn", 14);
    assert!(matches!(ctx, CompletionContext::Extend));
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

#[tokio::test]
async fn initialize_with_config() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize_with(
        &mut reader,
        &mut writer,
        serde_json::json!({
            "loadPaths": ["src/sass"],
            "importAliases": { "@sass": "src/sass" },
            "prependImports": ["variables"]
        }),
    )
    .await;
    assert!(resp["result"]["capabilities"].is_object());
}

#[tokio::test]
async fn initialize_with_empty_config() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize_with(&mut reader, &mut writer, serde_json::json!({})).await;
    assert!(resp["result"]["capabilities"].is_object());
}

#[test]
fn lsp_pos_to_byte_ascii() {
    let text = "abc\ndef\nghi";
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 0)), Some(0));
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 2)), Some(2));
    assert_eq!(lsp_pos_to_byte(text, Position::new(1, 0)), Some(4));
    assert_eq!(lsp_pos_to_byte(text, Position::new(1, 2)), Some(6));
    assert_eq!(lsp_pos_to_byte(text, Position::new(2, 1)), Some(9));
}

#[test]
fn lsp_pos_to_byte_multibyte() {
    // "ä" is 2 UTF-8 bytes, 1 UTF-16 code unit
    let text = "äbc";
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 0)), Some(0));
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 1)), Some(2)); // after ä
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 2)), Some(3)); // after b
}

#[test]
fn lsp_pos_to_byte_emoji() {
    // "😀" is 4 UTF-8 bytes, 2 UTF-16 code units (surrogate pair)
    let text = "😀b";
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 0)), Some(0));
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 2)), Some(4)); // after 😀
    assert_eq!(lsp_pos_to_byte(text, Position::new(0, 3)), Some(5)); // after b
}

#[test]
fn lsp_pos_to_byte_out_of_bounds() {
    let text = "ab";
    // Line 1 doesn't exist (no newline)
    assert_eq!(lsp_pos_to_byte(text, Position::new(1, 0)), None);
}

#[test]
fn apply_content_changes_insert() {
    let mut text = String::from("ab\ncd");
    let changes = vec![TextDocumentContentChangeEvent {
        range: Some(Range::new(Position::new(0, 1), Position::new(0, 1))),
        range_length: None,
        text: "X".into(),
    }];
    assert!(apply_content_changes(&mut text, changes));
    assert_eq!(text, "aXb\ncd");
}

#[test]
fn apply_content_changes_delete() {
    let mut text = String::from("abcd");
    let changes = vec![TextDocumentContentChangeEvent {
        range: Some(Range::new(Position::new(0, 1), Position::new(0, 3))),
        range_length: None,
        text: String::new(),
    }];
    assert!(apply_content_changes(&mut text, changes));
    assert_eq!(text, "ad");
}

#[test]
fn apply_content_changes_replace_across_lines() {
    let mut text = String::from("ab\ncd\nef");
    let changes = vec![TextDocumentContentChangeEvent {
        range: Some(Range::new(Position::new(0, 1), Position::new(1, 1))),
        range_length: None,
        text: "XY".into(),
    }];
    assert!(apply_content_changes(&mut text, changes));
    assert_eq!(text, "aXYd\nef");
}

#[test]
fn apply_content_changes_full_replacement() {
    let mut text = String::from("old");
    let changes = vec![TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: "new content".into(),
    }];
    assert!(apply_content_changes(&mut text, changes));
    assert_eq!(text, "new content");
}

#[test]
fn apply_content_changes_sequential() {
    let mut text = String::from("abc");
    // Two sequential changes: insert X at pos 1, then insert Y at pos 3
    // After first: "aXbc", after second: "aXbYc"
    let changes = vec![
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 1), Position::new(0, 1))),
            range_length: None,
            text: "X".into(),
        },
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 3), Position::new(0, 3))),
            range_length: None,
            text: "Y".into(),
        },
    ];
    assert!(apply_content_changes(&mut text, changes));
    assert_eq!(text, "aXbYc");
}

// ── Non-ASCII / UTF-16 tests ─────────────────────────────────────

#[test]
fn byte_to_lsp_pos_ascii() {
    let source = "abc\ndef";
    let li = sass_parser::line_index::LineIndex::new(source);
    // 'd' is at byte 4 (line 2, col 1)
    let (line, col) = byte_to_lsp_pos(source, &li, 4.into());
    assert_eq!((line, col), (1, 0));
}

#[test]
fn byte_to_lsp_pos_multibyte() {
    // 'é' is 2 bytes in UTF-8 but 1 UTF-16 code unit
    let source = "café\nx";
    let li = sass_parser::line_index::LineIndex::new(source);
    // 'x' is at byte 6 (c=1, a=1, f=1, é=2, \n=1)
    let (line, col) = byte_to_lsp_pos(source, &li, 6.into());
    assert_eq!((line, col), (1, 0));
    // end of "café" = byte 5, col should be 4 (c, a, f, é each 1 UTF-16 unit)
    let (line, col) = byte_to_lsp_pos(source, &li, 5.into());
    assert_eq!((line, col), (0, 4));
}

#[test]
fn byte_to_lsp_pos_surrogate_pair() {
    // '𝕊' (U+1D54A) is 4 bytes in UTF-8 and 2 UTF-16 code units (surrogate pair)
    let source = "a𝕊b\nx";
    let li = sass_parser::line_index::LineIndex::new(source);
    // 'b' is at byte 5 (a=1, 𝕊=4), should be UTF-16 col 3 (a=1, 𝕊=2)
    let (line, col) = byte_to_lsp_pos(source, &li, 5.into());
    assert_eq!((line, col), (0, 3));
}

#[test]
fn apply_content_changes_multibyte() {
    // LSP positions use UTF-16 columns. 'é' is 1 UTF-16 unit.
    let mut text = String::from("café");
    // Insert 'X' after 'f' (UTF-16 col 3)
    let changes = vec![TextDocumentContentChangeEvent {
        range: Some(Range::new(Position::new(0, 3), Position::new(0, 3))),
        range_length: None,
        text: "X".into(),
    }];
    assert!(apply_content_changes(&mut text, changes));
    assert_eq!(text, "cafXé");
}

#[test]
fn apply_content_changes_surrogate_pair() {
    // '𝕊' (U+1D54A) is 2 UTF-16 code units.
    let mut text = String::from("a𝕊b");
    // Delete 'b' at UTF-16 col 3 (a=1, 𝕊=2)
    let changes = vec![TextDocumentContentChangeEvent {
        range: Some(Range::new(Position::new(0, 3), Position::new(0, 4))),
        range_length: None,
        text: String::new(),
    }];
    assert!(apply_content_changes(&mut text, changes));
    assert_eq!(text, "a𝕊");
}

#[tokio::test]
async fn diagnostics_with_non_ascii_content() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Open a file with Ukrainian text and an intentional parse error
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///unicode.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": "$колір: #fff;\n.кнопка { color: $колір; }"
                }
            }
        }),
    )
    .await;

    let diag = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(diag["method"], "textDocument/publishDiagnostics");
    let diagnostics = diag["params"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.is_empty(),
        "valid SCSS with non-ASCII should produce no errors, got: {diagnostics:?}"
    );
}

#[tokio::test]
async fn incremental_sync_updates_diagnostics() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Open with valid SCSS
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///incr.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": "$x: 1;\n.a { color: red; }"
                }
            }
        }),
    )
    .await;

    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "valid SCSS should have 0 diagnostics");

    // Send incremental change: replace "red" with "blue"
    // "red" is at line 1, col 14..17
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///incr.scss", "version": 2 },
                "contentChanges": [{
                    "range": {
                        "start": { "line": 1, "character": 14 },
                        "end": { "line": 1, "character": 17 }
                    },
                    "text": "blue"
                }]
            }
        }),
    )
    .await;

    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "incremental edit should still be valid");
}

#[tokio::test]
async fn incremental_sync_hover_after_edit() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Open with a variable
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///incr2.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": "$color: red;\n.a { color: $color; }"
                }
            }
        }),
    )
    .await;
    let _ = recv_msg(&mut reader, &mut writer).await; // diagnostics

    // Incremental change: replace "red" with "blue" (line 0, col 8..11)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///incr2.scss", "version": 2 },
                "contentChanges": [{
                    "range": {
                        "start": { "line": 0, "character": 8 },
                        "end": { "line": 0, "character": 11 }
                    },
                    "text": "blue"
                }]
            }
        }),
    )
    .await;
    let _ = recv_msg(&mut reader, &mut writer).await; // diagnostics

    // Hover on $color reference — should show updated value "blue"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///incr2.scss" },
                "position": { "line": 1, "character": 16 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let contents = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        contents.contains("blue"),
        "hover should reflect incremental edit, got: {contents}"
    );
}

#[tokio::test]
async fn external_change_updates_module_graph() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Open a file
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ext.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": "$ext: 1;\n"
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Simulate an external change notification for a different file.
    // Since did_change_watched_files reads from disk and we can't
    // set up real files in this test, we verify that the notification
    // method is accepted without error by sending it as JSON-RPC.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": {
                "changes": [{
                    "uri": "file:///nonexistent.scss",
                    "type": 2
                }]
            }
        }),
    )
    .await;

    // The server should not crash; verify it still responds.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": "file:///ext.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(
        result.len(),
        1,
        "should still have 1 symbol after ext change"
    );
}

#[tokio::test]
async fn external_delete_does_not_crash() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Open a file
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///del.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": "$del: 1;\n"
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // External delete of a file that's not open (should be processed).
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": {
                "changes": [{
                    "uri": "file:///some_dep.scss",
                    "type": 3
                }]
            }
        }),
    )
    .await;

    // Small delay for the worker to process the delete task.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Server should still be alive.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 101,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": "file:///del.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 1, "should still have 1 symbol");
}

#[tokio::test]
async fn watched_files_skips_open_files() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$open: 1;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///open.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Notify file change for an open file — should be skipped
    // because open files are tracked by did_open/did_change.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": {
                "changes": [{
                    "uri": "file:///open.scss",
                    "type": 2
                }]
            }
        }),
    )
    .await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Server should still work correctly with the original content.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 102,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": "file:///open.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 1, "should still have the original symbol");
    assert_eq!(result[0]["name"], "open");
}

// ── Empty file handling ───────────────────────────────────────────

#[tokio::test]
async fn hover_on_empty_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///empty.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": ""
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 900,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///empty.scss" },
                "position": { "line": 0, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["result"].is_null(),
        "hover on empty file should be null"
    );
}

#[tokio::test]
async fn completion_on_empty_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///empty_comp.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": ""
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 901,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///empty_comp.scss" },
                "position": { "line": 0, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    // Should return either null or an empty/valid list, not crash
    let result = &resp["result"];
    assert!(
        result.is_null() || result.is_array() || result.is_object(),
        "completion on empty file should not crash"
    );
}

#[tokio::test]
async fn semantic_tokens_on_empty_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///empty_tokens.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": ""
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 902,
            "method": "textDocument/semanticTokens/full",
            "params": {
                "textDocument": { "uri": "file:///empty_tokens.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    if !result.is_null() {
        let data = result["data"].as_array().unwrap();
        assert!(data.is_empty(), "empty file should have no semantic tokens");
    }
}

// ---------------------------------------------------------------------------
// Color decorators
// ---------------------------------------------------------------------------

#[tokio::test]
async fn document_color_hex() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".btn { color: #ff0000; background: #0f0; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///color_hex.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 50,
            "method": "textDocument/documentColor",
            "params": {
                "textDocument": { "uri": "file:///color_hex.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let colors = resp["result"].as_array().unwrap();
    assert_eq!(colors.len(), 2, "should find two hex colors");

    // First color: #ff0000 → red
    let c0 = &colors[0]["color"];
    assert!((c0["red"].as_f64().unwrap() - 1.0).abs() < 0.01);
    assert!((c0["green"].as_f64().unwrap()).abs() < 0.01);
    assert!((c0["blue"].as_f64().unwrap()).abs() < 0.01);

    // Second color: #0f0 → green
    let c1 = &colors[1]["color"];
    assert!((c1["red"].as_f64().unwrap()).abs() < 0.01);
    assert!((c1["green"].as_f64().unwrap() - 1.0).abs() < 0.01);
    assert!((c1["blue"].as_f64().unwrap()).abs() < 0.01);
}

#[tokio::test]
async fn document_color_rgb_function() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".btn { color: rgb(0, 128, 255); }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///color_rgb.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 51,
            "method": "textDocument/documentColor",
            "params": {
                "textDocument": { "uri": "file:///color_rgb.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let colors = resp["result"].as_array().unwrap();
    assert_eq!(colors.len(), 1, "should find one rgb color");

    let c0 = &colors[0]["color"];
    assert!((c0["red"].as_f64().unwrap()).abs() < 0.01);
    assert!((c0["green"].as_f64().unwrap() - 0.502).abs() < 0.01);
    assert!((c0["blue"].as_f64().unwrap() - 1.0).abs() < 0.01);
}

#[tokio::test]
async fn color_presentation_returns_formats() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 52,
            "method": "textDocument/colorPresentation",
            "params": {
                "textDocument": { "uri": "file:///test.scss" },
                "color": { "red": 1.0, "green": 0.0, "blue": 0.0, "alpha": 1.0 },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 7 } }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let presentations = resp["result"].as_array().unwrap();
    assert!(
        presentations.len() >= 3,
        "should return at least 3 presentations (hex short, hex long, rgb, hsl)"
    );
    let labels: Vec<&str> = presentations
        .iter()
        .map(|p| p["label"].as_str().unwrap())
        .collect();
    assert!(labels.contains(&"#ff0000"), "should include hex format");
    assert!(
        labels.contains(&"rgb(255, 0, 0)"),
        "should include rgb format"
    );
}

#[tokio::test]
async fn document_color_hsl_function() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".btn { color: hsl(120, 100%, 50%); }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///color_hsl.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 53,
            "method": "textDocument/documentColor",
            "params": {
                "textDocument": { "uri": "file:///color_hsl.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let colors = resp["result"].as_array().unwrap();
    assert_eq!(colors.len(), 1, "should find one hsl color");

    let c0 = &colors[0]["color"];
    assert!((c0["red"].as_f64().unwrap()).abs() < 0.01);
    assert!((c0["green"].as_f64().unwrap() - 1.0).abs() < 0.01);
    assert!((c0["blue"].as_f64().unwrap()).abs() < 0.01);
}

#[tokio::test]
async fn document_color_named() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".btn { color: red; background: cornflowerblue; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///color_named.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 54,
            "method": "textDocument/documentColor",
            "params": {
                "textDocument": { "uri": "file:///color_named.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let colors = resp["result"].as_array().unwrap();
    assert_eq!(colors.len(), 2, "should find two named colors");

    // First: red
    let c0 = &colors[0]["color"];
    assert!((c0["red"].as_f64().unwrap() - 1.0).abs() < 0.01);
    assert!((c0["green"].as_f64().unwrap()).abs() < 0.01);
    assert!((c0["blue"].as_f64().unwrap()).abs() < 0.01);

    // Second: cornflowerblue (100, 149, 237)
    let c1 = &colors[1]["color"];
    assert!((c1["red"].as_f64().unwrap() - 0.392).abs() < 0.01);
    assert!((c1["green"].as_f64().unwrap() - 0.584).abs() < 0.01);
    assert!((c1["blue"].as_f64().unwrap() - 0.929).abs() < 0.01);
}

#[tokio::test]
async fn initialize_reports_color_provider_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(
        caps["colorProvider"], true,
        "server should advertise color provider"
    );
}

// ---------------------------------------------------------------------------
// Folding ranges
// ---------------------------------------------------------------------------

async fn get_folds(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    scss: &str,
    id: u64,
) -> Vec<serde_json::Value> {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(reader, writer).await;

    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/foldingRange",
            "params": {
                "textDocument": { "uri": uri }
            }
        }),
    )
    .await;

    let resp = recv_msg(reader, writer).await;
    resp["result"].as_array().unwrap().clone()
}

#[tokio::test]
async fn folding_range_rule_set() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .btn {      ← line 0
    //   color: red;   ← line 1
    //   font-size: 14px;  ← line 2
    // }            ← line 3  (closing brace stays visible)
    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_rule.scss",
        ".btn {\n  color: red;\n  font-size: 14px;\n}\n",
        60,
    )
    .await;
    assert_eq!(folds.len(), 1, "one fold for the rule set");
    assert_eq!(folds[0]["startLine"], 0);
    assert_eq!(folds[0]["endLine"], 2, "end_line excludes closing brace");
    assert!(folds[0]["kind"].is_null(), "block folds have no kind");
}

#[tokio::test]
async fn folding_range_nested_rules() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .parent {     ← line 0
    //   .child {    ← line 1
    //     color: red; ← line 2
    //   }           ← line 3
    // }             ← line 4
    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_nested.scss",
        ".parent {\n  .child {\n    color: red;\n  }\n}\n",
        61,
    )
    .await;
    assert_eq!(folds.len(), 2, "two folds: parent and child rule sets");
    // Parent fold: lines 0..3 (} on line 4 visible)
    assert_eq!(folds[0]["startLine"], 0);
    assert_eq!(folds[0]["endLine"], 3);
    // Child fold: lines 1..2 (} on line 3 visible)
    assert_eq!(folds[1]["startLine"], 1);
    assert_eq!(folds[1]["endLine"], 2);
}

#[tokio::test]
async fn folding_range_at_rules() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_at_rules.scss",
        "@mixin flex($dir) {\n  display: flex;\n  flex-direction: $dir;\n}\n\n\
         @media (min-width: 768px) {\n  .container {\n    width: 750px;\n  }\n}\n",
        62,
    )
    .await;
    // @mixin, @media, .container inside @media
    assert_eq!(
        folds.len(),
        3,
        "three folds: mixin, media, and inner rule set"
    );
}

#[tokio::test]
async fn folding_range_multiline_comment() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // /*          ← line 0
    //  * Multi    ← line 1
    //  * comment  ← line 2
    //  */         ← line 3
    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_comment.scss",
        "/*\n * Multi-line\n * comment\n */\n.btn { color: red; }\n",
        63,
    )
    .await;
    let comment_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("comment"))
        .collect();
    assert_eq!(comment_folds.len(), 1, "one comment fold");
    assert_eq!(comment_folds[0]["startLine"], 0);
    assert_eq!(comment_folds[0]["endLine"], 2, "end_line excludes */ line");
}

#[tokio::test]
async fn folding_range_region_markers() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_region.scss",
        "// #region Variables\n$color: red;\n$size: 14px;\n// #endregion\n",
        64,
    )
    .await;
    let region_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("region"))
        .collect();
    assert_eq!(region_folds.len(), 1, "one region fold");
    assert_eq!(region_folds[0]["startLine"], 0);
    assert_eq!(region_folds[0]["endLine"], 3);
}

#[tokio::test]
async fn folding_range_single_line_not_folded() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_single.scss",
        ".btn { color: red; }\n",
        65,
    )
    .await;
    assert_eq!(folds.len(), 0, "single-line rule should not produce a fold");
}

#[tokio::test]
async fn folding_range_consecutive_line_comments() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_consec.scss",
        "// First line\n// Second line\n// Third line\n.btn {\n  color: red;\n}\n",
        66,
    )
    .await;
    let comment_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("comment"))
        .collect();
    assert_eq!(comment_folds.len(), 1, "consecutive comments fold together");
    assert_eq!(comment_folds[0]["startLine"], 0);
    assert_eq!(comment_folds[0]["endLine"], 2);
}

#[tokio::test]
async fn folding_range_unmatched_region_ignored() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_unmatched.scss",
        "// #region No End\n$color: red;\n",
        67,
    )
    .await;
    let region_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("region"))
        .collect();
    assert_eq!(region_folds.len(), 0, "unmatched #region produces no fold");
}

#[tokio::test]
async fn folding_range_nested_regions() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_nested_region.scss",
        "// #region Outer\n// #region Inner\n$x: 1;\n// #endregion\n$y: 2;\n// #endregion\n",
        68,
    )
    .await;
    let region_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("region"))
        .collect();
    assert_eq!(region_folds.len(), 2, "nested regions produce two folds");
}

#[tokio::test]
async fn folding_range_empty_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(&mut reader, &mut writer, "file:///fold_empty.scss", "", 69).await;
    assert_eq!(folds.len(), 0, "empty file produces no folds");
}

#[tokio::test]
async fn initialize_reports_folding_range_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(
        caps["foldingRangeProvider"], true,
        "server should advertise folding range provider"
    );
}

// ── Document Highlights ─────────────────────────────────────────────

async fn get_highlights(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    scss: &str,
    id: u64,
    line: u32,
    character: u32,
) -> Vec<Value> {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(reader, writer).await;

    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/documentHighlight",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        }),
    )
    .await;

    let resp = recv_msg(reader, writer).await;
    resp["result"].as_array().cloned().unwrap_or_default()
}

#[tokio::test]
async fn highlight_variable_from_definition() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.a { color: $color; }\n.b { border-color: $color; }\n";
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_var_def.scss",
        scss,
        80,
        0,
        1,
    )
    .await;

    assert_eq!(highlights.len(), 3, "definition + 2 references");
    assert_eq!(highlights[0]["kind"], 3, "definition is Write (3)");
    assert_eq!(highlights[1]["kind"], 2, "reference is Read (2)");
    assert_eq!(highlights[2]["kind"], 2, "reference is Read (2)");
}

#[tokio::test]
async fn highlight_variable_from_reference() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.a { color: $color; }\n";
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_var_ref.scss",
        scss,
        81,
        1,
        15,
    )
    .await;

    assert_eq!(highlights.len(), 2, "definition + 1 reference");
    assert_eq!(highlights[0]["kind"], 3, "definition is Write");
    assert_eq!(highlights[1]["kind"], 2, "reference is Read");
}

#[tokio::test]
async fn highlight_function() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function double($n) { @return $n * 2; }\n.a { width: double(5px); }\n";
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_func.scss",
        scss,
        82,
        1,
        13,
    )
    .await;

    assert_eq!(highlights.len(), 2, "definition + 1 call");
    assert_eq!(highlights[0]["kind"], 3, "definition is Write");
    assert_eq!(highlights[1]["kind"], 2, "call is Read");
}

#[tokio::test]
async fn highlight_mixin() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin btn { display: block; }\n.card { @include btn; }\n";
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_mixin.scss",
        scss,
        83,
        1,
        16,
    )
    .await;

    assert_eq!(highlights.len(), 2, "definition + 1 include");
    assert_eq!(highlights[0]["kind"], 3, "definition is Write");
    assert_eq!(highlights[1]["kind"], 2, "include is Read");
}

#[tokio::test]
async fn highlight_placeholder() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "%base { display: block; }\n.btn { @extend %base; }\n";
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_placeholder.scss",
        scss,
        84,
        0,
        1,
    )
    .await;

    assert_eq!(highlights.len(), 2, "definition + 1 extend");
    assert_eq!(highlights[0]["kind"], 3, "definition is Write");
    assert_eq!(highlights[1]["kind"], 2, "extend is Read");
}

#[tokio::test]
async fn highlight_same_name_different_kind() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // $size variable and size() function — same name, different kind
    let scss = "$size: 10px;\n@function size() { @return 20px; }\n.a { width: $size; }\n";
    // Cursor on $size reference (line 2, char 13)
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_diff_kind.scss",
        scss,
        85,
        2,
        13,
    )
    .await;

    assert_eq!(highlights.len(), 2, "only variable matches, not function");
    // All should be variable-related
    assert_eq!(highlights[0]["range"]["start"]["line"], 0, "variable def");
    assert_eq!(highlights[1]["range"]["start"]["line"], 2, "variable ref");
}

#[tokio::test]
async fn highlight_returns_null_on_non_symbol() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".btn { color: red; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hl_none.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 86,
            "method": "textDocument/documentHighlight",
            "params": {
                "textDocument": { "uri": "file:///hl_none.scss" },
                "position": { "line": 0, "character": 8 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(resp["result"].is_null(), "non-symbol should return null");
}

#[tokio::test]
async fn highlight_orphan_definition() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$unused: 42;\n";
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_orphan.scss",
        scss,
        87,
        0,
        1,
    )
    .await;

    assert_eq!(highlights.len(), 1, "definition only, no references");
    assert_eq!(highlights[0]["kind"], 3, "definition is Write");
}

#[tokio::test]
async fn highlight_namespaced_reference_returns_null() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Cursor on ns.$color — namespaced ref should return null (cross-file)
    let scss = "@use 'colors' as ns;\n.a { color: ns.$color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hl_ns.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 88,
            "method": "textDocument/documentHighlight",
            "params": {
                "textDocument": { "uri": "file:///hl_ns.scss" },
                "position": { "line": 1, "character": 16 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["result"].is_null(),
        "namespaced reference should return null"
    );
}

#[tokio::test]
async fn highlight_variable_multiple_definitions() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$x: 1;\n$x: 2;\n.a { width: $x; }\n";
    let highlights = get_highlights(
        &mut reader,
        &mut writer,
        "file:///hl_multi_def.scss",
        scss,
        89,
        2,
        13,
    )
    .await;

    assert_eq!(highlights.len(), 3, "2 definitions + 1 reference");
    assert_eq!(highlights[0]["kind"], 3, "first def is Write");
    assert_eq!(highlights[1]["kind"], 3, "second def is Write");
    assert_eq!(highlights[2]["kind"], 2, "reference is Read");
}

#[tokio::test]
async fn initialize_reports_document_highlight_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(
        caps["documentHighlightProvider"], true,
        "server should advertise document highlight provider"
    );
}

// ── Selection ranges ────────────────────────────────────────────────

async fn get_selection_ranges(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    scss: &str,
    id: u64,
    positions: Vec<(u32, u32)>,
) -> Vec<Value> {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(reader, writer).await;

    let lsp_positions: Vec<Value> = positions
        .iter()
        .map(|(line, character)| serde_json::json!({ "line": line, "character": character }))
        .collect();

    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/selectionRange",
            "params": {
                "textDocument": { "uri": uri },
                "positions": lsp_positions
            }
        }),
    )
    .await;

    let resp = recv_msg(reader, writer).await;
    resp["result"].as_array().cloned().unwrap_or_default()
}

/// Flatten a nested SelectionRange into a list of ranges from innermost to outermost.
fn flatten_selection_range(sr: &Value) -> Vec<&Value> {
    let mut result = Vec::new();
    let mut current = sr;
    loop {
        result.push(&current["range"]);
        if current["parent"].is_null() {
            break;
        }
        current = &current["parent"];
    }
    result
}

#[tokio::test]
async fn selection_range_property_value() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .btn {         ← line 0
    //   color: red;  ← line 1, cursor on "red" (col 9)
    // }              ← line 2
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_prop.scss",
        ".btn {\n  color: red;\n}\n",
        90,
        vec![(1, 9)],
    )
    .await;
    assert_eq!(results.len(), 1, "one result for one position");

    let chain = flatten_selection_range(&results[0]);
    assert!(chain.len() >= 3, "at least token → declaration → rule set");

    // Innermost should be "red" token
    let innermost = chain[0];
    assert_eq!(innermost["start"]["line"], 1);
    assert_eq!(innermost["start"]["character"], 9);
    assert_eq!(innermost["end"]["line"], 1);
    assert_eq!(innermost["end"]["character"], 12);

    // Outermost should cover the entire file (root)
    let outermost = chain.last().unwrap();
    assert_eq!(outermost["start"]["line"], 0);
    assert_eq!(outermost["start"]["character"], 0);
}

#[tokio::test]
async fn selection_range_variable_reference() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // $color: red;       ← line 0
    // .a { color: $color; } ← line 1, cursor on $color (col 13)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_var.scss",
        "$color: red;\n.a { color: $color; }\n",
        91,
        vec![(1, 13)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    assert!(chain.len() >= 3, "at least token → declaration → rule set");

    // Innermost is "color" IDENT token ($ is a separate DOLLAR token)
    let innermost = chain[0];
    assert_eq!(innermost["start"]["line"], 1);
    assert_eq!(innermost["start"]["character"], 13);
    assert_eq!(innermost["end"]["line"], 1);
    assert_eq!(innermost["end"]["character"], 18);
}

#[tokio::test]
async fn selection_range_nested_rules() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .parent {             ← line 0
    //   .child {            ← line 1
    //     color: red;       ← line 2, cursor on "color" (col 4)
    //   }                   ← line 3
    // }                     ← line 4
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_nested.scss",
        ".parent {\n  .child {\n    color: red;\n  }\n}\n",
        92,
        vec![(2, 4)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // token → declaration → child rule set → parent rule set → root
    assert!(
        chain.len() >= 4,
        "at least token → declaration → child → parent → root, got {}",
        chain.len()
    );
}

#[tokio::test]
async fn selection_range_multiple_positions() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_multi.scss",
        ".a { color: red; }\n.b { font-size: 14px; }\n",
        93,
        vec![(0, 5), (1, 5)],
    )
    .await;
    assert_eq!(results.len(), 2, "one result per position");
}

#[tokio::test]
async fn selection_range_selector() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .btn { color: red; }  ← cursor on ".btn" (col 1)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_selector.scss",
        ".btn { color: red; }\n",
        94,
        vec![(0, 1)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    assert!(
        chain.len() >= 2,
        "at least token → selector → rule set → root"
    );
}

#[tokio::test]
async fn selection_range_at_rule() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // @mixin flex($dir) {          ← line 0
    //   display: flex;             ← line 1, cursor on "flex" (col 11)
    //   flex-direction: $dir;      ← line 2
    // }                            ← line 3
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_at.scss",
        "@mixin flex($dir) {\n  display: flex;\n  flex-direction: $dir;\n}\n",
        95,
        vec![(1, 11)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // token → declaration → mixin rule → root
    assert!(
        chain.len() >= 3,
        "at least token → declaration → mixin → root, got {}",
        chain.len()
    );
}

#[tokio::test]
async fn selection_range_empty_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_empty.scss",
        "",
        96,
        vec![(0, 0)],
    )
    .await;
    // Fallback: one result per position even if no token found
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["range"]["start"]["line"], 0);
    assert_eq!(results[0]["range"]["start"]["character"], 0);
}

#[tokio::test]
async fn selection_range_cursor_at_eof() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // ".a {}\n" — cursor past the last char (line 1, col 0)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_eof.scss",
        ".a {}\n",
        97,
        vec![(1, 0)],
    )
    .await;
    assert_eq!(
        results.len(),
        1,
        "must return one result to keep index correspondence"
    );
}

#[tokio::test]
async fn selection_range_comment() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // /* hello */     ← line 0, cursor inside comment (col 4)
    // .a { color: red; }
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_comment.scss",
        "/* hello */\n.a { color: red; }\n",
        98,
        vec![(0, 4)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // Comment token → root
    assert!(chain.len() >= 2, "at least comment token → root");
    // Innermost should cover the comment
    assert_eq!(chain[0]["start"]["line"], 0);
    assert_eq!(chain[0]["start"]["character"], 0);
    assert_eq!(chain[0]["end"]["character"], 11);
}

#[tokio::test]
async fn selection_range_full_chain_verification() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .a { color: red; }
    // Cursor on "red" (line 0, col 13)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_chain.scss",
        ".a { color: red; }\n",
        99,
        vec![(0, 13)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // Verify every range is strictly contained within its parent
    for i in 0..chain.len() - 1 {
        let inner = chain[i];
        let outer = chain[i + 1];
        let inner_start = (
            inner["start"]["line"].as_u64().unwrap(),
            inner["start"]["character"].as_u64().unwrap(),
        );
        let inner_end = (
            inner["end"]["line"].as_u64().unwrap(),
            inner["end"]["character"].as_u64().unwrap(),
        );
        let outer_start = (
            outer["start"]["line"].as_u64().unwrap(),
            outer["start"]["character"].as_u64().unwrap(),
        );
        let outer_end = (
            outer["end"]["line"].as_u64().unwrap(),
            outer["end"]["character"].as_u64().unwrap(),
        );
        assert!(
            outer_start <= inner_start && inner_end <= outer_end,
            "range {i} must be contained within range {}: {:?} not in {:?}",
            i + 1,
            (inner_start, inner_end),
            (outer_start, outer_end),
        );
    }

    // Outermost must cover entire file
    let outermost = chain.last().unwrap();
    assert_eq!(outermost["start"]["line"], 0);
    assert_eq!(outermost["start"]["character"], 0);
}

#[tokio::test]
async fn selection_range_interpolation() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .#{$var} { color: red; }  ← cursor on "$var" (col 4)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_interp.scss",
        ".#{$var} { color: red; }\n",
        100,
        vec![(0, 4)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    assert!(
        chain.len() >= 3,
        "at least token → interpolation → selector → root, got {}",
        chain.len()
    );
}

#[tokio::test]
async fn selection_range_whitespace_only() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_ws.scss",
        "   \n  \n",
        101,
        vec![(0, 1)],
    )
    .await;
    assert_eq!(
        results.len(),
        1,
        "must return one result even for whitespace-only file"
    );
}

#[tokio::test]
async fn initialize_reports_selection_range_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(
        caps["selectionRangeProvider"], true,
        "server should advertise selection range provider"
    );
}

// ── Semantic diagnostics tests ─────────────────────────────────────

/// Open a document and return the published diagnostics.
async fn open_and_get_diagnostics(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    text: &str,
    version: i32,
) -> Vec<Value> {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "scss",
                    "version": version,
                    "text": text
                }
            }
        }),
    )
    .await;
    let notif = recv_msg(reader, writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    notif["params"]["diagnostics"].as_array().unwrap().clone()
}

// ── Arg count tests ────────────────────────────────────────────────

#[tokio::test]
async fn semantic_too_few_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args1.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "should report too few args");
    assert!(
        semantic[0]["message"]
            .as_str()
            .unwrap()
            .contains("at least 2"),
    );
}

#[tokio::test]
async fn semantic_exact_args_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args2.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert!(
        semantic.is_empty(),
        "exact args should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_too_many_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2, 3); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args3.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "should report too many args");
    assert!(
        semantic[0]["message"]
            .as_str()
            .unwrap()
            .contains("at most 2"),
    );
}

#[tokio::test]
async fn semantic_args_with_defaults_ok() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f($a, $b: 10px) { @return $a + $b; }\n.x { width: f(1); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args4.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert!(
        semantic.is_empty(),
        "call with default-covered args should be ok"
    );
}

#[tokio::test]
async fn semantic_args_with_rest_param() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f($a, $rest...) { @return $a; }\n.x { width: f(1, 2, 3, 4); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args5.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert!(semantic.is_empty(), "rest param should accept any count");
}

#[tokio::test]
async fn semantic_mixin_too_few_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@mixin flex($dir, $wrap) { display: flex; }\n.x { @include flex(); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args6.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "mixin with too few args should error");
}

#[tokio::test]
async fn semantic_zero_param_called_with_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f() { @return 1; }\n.x { width: f(42); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args7.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "zero-param function called with args");
}

// ── Undefined reference tests ──────────────────────────────────────

#[tokio::test]
async fn semantic_undefined_variable() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { color: $undefined-var; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef1.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert_eq!(semantic.len(), 1);
    assert!(
        semantic[0]["message"]
            .as_str()
            .unwrap()
            .contains("undefined-var"),
    );
    assert_eq!(semantic[0]["severity"], 2, "should be WARNING (2)");
}

#[tokio::test]
async fn semantic_defined_variable_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "$color: red;\n.x { color: $color; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef2.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "defined variable should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_css_var_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { color: var(--custom); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef3.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        semantic.is_empty(),
        "CSS var() should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_css_calc_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { width: calc(100% - 20px); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef4.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        semantic.is_empty(),
        "CSS calc() should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_undefined_function() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { width: nonexistent-fn(1); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef5.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert_eq!(semantic.len(), 1);
}

#[tokio::test]
async fn semantic_undefined_mixin() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { @include nonexistent-mixin(); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef6.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-mixin"))
        .collect();
    assert_eq!(semantic.len(), 1);
}

#[tokio::test]
async fn semantic_defined_function_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function double($n) { @return $n * 2; }\n.x { width: double(5); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef7.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "defined function should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_defined_mixin_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@mixin bold { font-weight: bold; }\n.x { @include bold; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef8.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "defined mixin should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_placeholder_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { @extend %placeholder; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef9.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "placeholder @extend should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_diagnostics_have_codes() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f($a) { @return $a; }\n.x { width: f(); color: $nope; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///codes.scss", text, 1).await;

    let with_codes: Vec<_> = diags.iter().filter(|d| d["code"].is_string()).collect();
    assert!(
        with_codes.len() >= 2,
        "should have at least arg-count + undefined diagnostics with codes"
    );
}

#[tokio::test]
async fn semantic_import_suppresses_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@import 'variables';\n.x { color: $imported-var; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///suppress1.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert!(
        semantic.is_empty(),
        "files with @import should suppress undefined warnings"
    );
}

#[tokio::test]
async fn semantic_function_param_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function clamp-val($min, $max, $val) { @return max($min, min($max, $val)); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///param1.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert!(
        undef.is_empty(),
        "function parameters should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_mixin_param_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@mixin flex($dir, $wrap: nowrap) { flex-direction: $dir; flex-wrap: $wrap; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///param2.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert!(
        undef.is_empty(),
        "mixin parameters should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_each_loop_var_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "$list: a, b, c;\n@each $item in $list { .#{$item} { display: block; } }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///loop1.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"].as_str() == Some("undefined-variable")
                && d["message"].as_str().is_some_and(|m| m.contains("item"))
        })
        .collect();
    assert!(
        undef.is_empty(),
        "@each loop variable should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_for_loop_var_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@for $i from 1 through 3 { .col-#{$i} { width: percentage($i / 12); } }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///loop2.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"].as_str() == Some("undefined-variable")
                && d["message"].as_str().is_some_and(|m| m.contains("`i`"))
        })
        .collect();
    assert!(
        undef.is_empty(),
        "@for loop variable should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_gradient_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { background: linear-gradient(to right, red, blue); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///css1.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        undef.is_empty(),
        "CSS linear-gradient() should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_transform_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { transform: translateX(10px) rotate(45deg) scale(1.5); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///css2.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        undef.is_empty(),
        "CSS transform functions should not trigger undefined"
    );
}
