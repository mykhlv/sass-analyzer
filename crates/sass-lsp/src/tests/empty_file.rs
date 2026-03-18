use super::*;

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
