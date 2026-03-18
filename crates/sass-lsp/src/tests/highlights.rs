use super::*;

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
