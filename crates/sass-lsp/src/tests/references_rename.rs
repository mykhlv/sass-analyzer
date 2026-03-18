use super::*;

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
