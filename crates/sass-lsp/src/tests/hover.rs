use super::*;

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
