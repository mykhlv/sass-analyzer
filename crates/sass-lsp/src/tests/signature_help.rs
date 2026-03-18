use super::*;

use tower_lsp_server::ls_types::ParameterLabel;

use crate::signature_help::parse_param_labels;

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
