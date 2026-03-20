use super::*;

use crate::semantic_tokens::{MOD_DECLARATION, TOK_VARIABLE};

#[tokio::test]
async fn initialize_reports_hover_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;
    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["hoverProvider"], true);
}

#[tokio::test]
async fn initialize_returns_capabilities() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["textDocumentSync"], 1);

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
