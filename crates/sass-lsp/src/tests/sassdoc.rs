use super::*;

// ── SassDoc integration tests ──────────────────────────────────────

#[tokio::test]
async fn hover_sassdoc_param_and_return() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
/// Doubles a number.
/// @param {Number} $n - The number to double
/// @return {Number} The doubled value
@function double($n) { @return $n * 2; }
.a { width: double(5); }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_sassdoc.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Hover on "double" in the call (line 4, character 14)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_sassdoc.scss" },
                "position": { "line": 4, "character": 14 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("**Parameters:**"),
        "hover should show structured params: {content}"
    );
    assert!(
        content.contains("`$n`"),
        "hover should show param name: {content}"
    );
    assert!(
        content.contains("`{Number}`"),
        "hover should show param type: {content}"
    );
    assert!(
        content.contains("**@return**"),
        "hover should show @return: {content}"
    );
}

#[tokio::test]
async fn hover_sassdoc_deprecated() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
/// @deprecated Use new-mixin instead
@mixin old-mixin { color: red; }
.a { @include old-mixin; }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_sassdoc_dep.scss",
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
            "id": 101,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_sassdoc_dep.scss" },
                "position": { "line": 2, "character": 14 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("**@deprecated**"),
        "hover should show deprecated: {content}"
    );
    assert!(
        content.contains("Use new-mixin instead"),
        "hover should show deprecation message: {content}"
    );
}

#[tokio::test]
async fn hover_sassdoc_example() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
/// Responsive breakpoint mixin.
/// @param {String} $bp - Breakpoint name
/// @example
///   @include respond-to(mobile) { display: none; }
@mixin respond-to($bp) { @content; }
.a { @include respond-to(mobile); }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_sassdoc_ex.scss",
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
            "id": 102,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_sassdoc_ex.scss" },
                "position": { "line": 5, "character": 14 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("**@example**"),
        "hover should show @example: {content}"
    );
    assert!(
        content.contains("```scss"),
        "hover should contain code block: {content}"
    );
}

#[tokio::test]
async fn signature_help_sassdoc_param_docs() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
/// @param {Color} $color - The base color
/// @param {Number} $amount - Adjustment amount
@function adjust($color, $amount) { @return $color; }
.a { color: adjust(red, 10%); }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///sig_sassdoc.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after the comma, on "10%" (line 3, character 24)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 103,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": "file:///sig_sassdoc.scss" },
                "position": { "line": 3, "character": 25 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let sig = &resp["result"]["signatures"][0];
    let params = sig["parameters"].as_array().unwrap();
    assert_eq!(params.len(), 2);

    // First param should have documentation
    let doc0 = params[0]["documentation"]["value"].as_str().unwrap();
    assert!(
        doc0.contains("The base color"),
        "param 0 doc should contain description: {doc0}"
    );

    // Second param should have documentation
    let doc1 = params[1]["documentation"]["value"].as_str().unwrap();
    assert!(
        doc1.contains("Adjustment amount"),
        "param 1 doc should contain description: {doc1}"
    );
}

#[tokio::test]
async fn hover_plain_doc_still_works() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "/// Just a simple description\n$color: red;\n.a { color: $color; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///hover_plain_doc.scss",
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
            "id": 104,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///hover_plain_doc.scss" },
                "position": { "line": 2, "character": 14 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let content = resp["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        content.contains("Just a simple description"),
        "plain doc should still work: {content}"
    );
    assert!(
        !content.contains("**Parameters:**"),
        "plain doc should not have structured sections: {content}"
    );
}
