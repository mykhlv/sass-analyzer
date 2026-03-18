use super::*;

use crate::convert::byte_to_lsp_pos;

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

// ── compute_diff_edit tests ──────────────────────────────────────

#[test]
fn diff_edit_insert() {
    use crate::compute_diff_edit;
    let old = ".a { color: red; }";
    let (green, errors) = sass_parser::parse_scss(old);
    let new = ".a { color: blue; }";
    let edit = compute_diff_edit(&green, &errors, old, new).unwrap();
    assert_eq!(u32::from(edit.edit.offset), 12); // "red" starts at byte 12
    assert_eq!(u32::from(edit.edit.delete), 3); // "red" = 3 bytes
    assert_eq!(u32::from(edit.edit.insert_len), 4); // "blue" = 4 bytes
}

#[test]
fn diff_edit_identical() {
    use crate::compute_diff_edit;
    let text = ".a { color: red; }";
    let (green, errors) = sass_parser::parse_scss(text);
    assert!(compute_diff_edit(&green, &errors, text, text).is_none());
}

#[test]
fn diff_edit_append() {
    use crate::compute_diff_edit;
    let old = ".a { }";
    let (green, errors) = sass_parser::parse_scss(old);
    let new = ".a { }\n.b { }";
    let edit = compute_diff_edit(&green, &errors, old, new).unwrap();
    assert_eq!(u32::from(edit.edit.offset), 6);
    assert_eq!(u32::from(edit.edit.delete), 0);
    assert_eq!(u32::from(edit.edit.insert_len), 7); // "\n.b { }"
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

    // Send full-text change: replace "red" with "blue"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///incr.scss", "version": 2 },
                "contentChanges": [{
                    "text": "$x: 1;\n.a { color: blue; }"
                }]
            }
        }),
    )
    .await;

    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "full-sync edit should still be valid");
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

    // Full-text change: replace "red" with "blue"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///incr2.scss", "version": 2 },
                "contentChanges": [{
                    "text": "$color: blue;\n.a { color: $color; }"
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

// ── max_file_size tests ──────────────────────────────────────────

#[tokio::test]
async fn did_open_exceeding_max_file_size() {
    let (mut reader, mut writer) = spawn_server();
    // Set maxFileSize to 10 bytes — anything larger is skipped.
    let _resp = do_initialize_with(
        &mut reader,
        &mut writer,
        serde_json::json!({ "maxFileSize": 10 }),
    )
    .await;

    // Open a file that exceeds the limit (> 10 bytes).
    let large_text = "$variable: red;\n.a { color: blue; }";
    assert!(large_text.len() > 10);
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///big.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": large_text
                }
            }
        }),
    )
    .await;

    // The oversized file should be silently skipped (no diagnostics published).
    // Open a small valid file to prove the server is still alive.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///small.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": "$a: 1;\n"
                }
            }
        }),
    )
    .await;

    let diag = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(diag["method"], "textDocument/publishDiagnostics");
    // The diagnostics must be for the small file, not the big one.
    assert!(
        diag["params"]["uri"].as_str().unwrap().contains("small"),
        "expected diagnostics for small.scss, got: {}",
        diag["params"]["uri"]
    );
    let diagnostics = diag["params"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.is_empty(),
        "valid small file should have 0 errors"
    );

    // Verify hover on the oversized file returns null (not indexed).
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///big.scss" },
                "position": { "line": 0, "character": 1 }
            }
        }),
    )
    .await;

    let resp = recv_response(&mut reader, &mut writer, 10).await;
    assert!(
        resp["result"].is_null(),
        "hover on oversized file should return null, got: {:?}",
        resp["result"]
    );
}

// ── Configuration & command tests ────────────────────────────────

#[tokio::test]
async fn did_change_configuration_updates_resolver() {
    let dir = std::env::temp_dir().join(format!("sass_cfg_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;

    // Send a configuration change with loadPaths
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeConfiguration",
            "params": {
                "settings": {
                    "sass-analyzer": {
                        "loadPaths": ["vendor/scss"],
                        "importAliases": {}
                    }
                }
            }
        }),
    )
    .await;

    // Server should accept the notification and still respond to requests
    let scss = "$cfg: 1;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///cfg_test.scss",
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
            "id": 200,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": "file:///cfg_test.scss" }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 1, "server still works after config change");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn execute_command_check_workspace() {
    let dir = std::env::temp_dir().join(format!("sass_cmd_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("main.scss"), "$x: 1;\n").unwrap();

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;

    // Execute checkWorkspace command — should succeed (return Ok(None))
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 201,
            "method": "workspace/executeCommand",
            "params": {
                "command": "sass-analyzer.checkWorkspace",
                "arguments": []
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["error"].is_null(),
        "checkWorkspace should not return error"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn execute_command_unknown_returns_error() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 202,
            "method": "workspace/executeCommand",
            "params": {
                "command": "sass-analyzer.nonExistent",
                "arguments": []
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        !resp["error"].is_null(),
        "unknown command should return method_not_found"
    );
}

#[tokio::test]
async fn did_change_full_sync_updates_correctly() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Open: "$x: 1;\n$y: 2;\n"
    let scss = "$x: 1;\n$y: 2;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///multi_edit.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Full-text change: both values updated
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///multi_edit.scss", "version": 2 },
                "contentChanges": [{
                    "text": "$x: 10;\n$y: 20;\n"
                }]
            }
        }),
    )
    .await;

    let notif = recv_msg(&mut reader, &mut writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "full-sync edit should produce valid SCSS");

    // Verify the edit took effect by hovering on $x
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 203,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///multi_edit.scss" },
                "position": { "line": 0, "character": 1 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let hover_text = resp["result"]["contents"]["value"].as_str().unwrap_or("");
    assert!(
        hover_text.contains("10"),
        "hover should reflect the updated value '10', got: {hover_text}"
    );
}
