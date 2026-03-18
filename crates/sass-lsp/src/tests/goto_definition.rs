use super::*;

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
