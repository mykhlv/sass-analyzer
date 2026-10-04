//! Code actions through the server, on indented-Sass (`.sass`) documents.
//!
//! The server picks the statement syntax from the document's extension, so these
//! go through `textDocument/codeAction` rather than calling the actions directly.

use expect_test::expect;

use super::*;

/// Request code actions for `range` until one titled `title_prefix…` shows up
/// (the module graph indexes in the background), and return its edits for `uri`.
async fn code_action_edits(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    range: &Value,
    diagnostics: &Value,
    title_prefix: &str,
) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut id = 900u64;
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "no `{title_prefix}` code action was offered"
        );
        send_msg(
            writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "textDocument/codeAction",
                "params": {
                    "textDocument": { "uri": uri },
                    "range": range,
                    "context": { "diagnostics": diagnostics }
                }
            }),
        )
        .await;
        let resp = recv_response(reader, writer, id).await;
        id += 1;
        let action = resp["result"].as_array().and_then(|actions| {
            actions.iter().find(|a| {
                a["title"]
                    .as_str()
                    .is_some_and(|t| t.starts_with(title_prefix))
            })
        });
        if let Some(action) = action {
            return action["edit"]["changes"][uri].as_array().unwrap().clone();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn sass_auto_import_writes_use_without_semicolon() {
    let dir = std::env::temp_dir().join(format!("sass_autoimport_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let colors_path = dir.join("_colors.sass");
    let main_path = dir.join("main.sass");
    let colors = "$primary: red\n";
    let main = "@use \"sass:math\"\n.a\n  color: $primary\n  width: math.div(4px, 2)\n";
    std::fs::write(&colors_path, colors).unwrap();
    std::fs::write(&main_path, main).unwrap();
    let colors_uri = file_uri(&colors_path);
    let main_uri = file_uri(&main_path);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;
    open_document(&mut writer, &colors_uri, colors).await;
    open_document(&mut writer, &main_uri, main).await;

    let range = serde_json::json!({
        "start": { "line": 2, "character": 9 },
        "end": { "line": 2, "character": 17 }
    });
    let diagnostics = serde_json::json!([{
        "range": range,
        "message": "undefined variable `primary`",
        "code": "undefined-variable",
        "data": { "name": "primary", "kind": "variable" }
    }]);
    let edits = code_action_edits(
        &mut reader,
        &mut writer,
        &main_uri,
        &range,
        &diagnostics,
        "Add `@use",
    )
    .await;

    let result = apply_lsp_edits(main, &edits);
    expect![[r#"
        @use "sass:math"
        @use "colors"
        .a
          color: colors.$primary
          width: math.div(4px, 2)
    "#]]
    .assert_eq(&result);
    assert_parses_as_sass(&result);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sass_extract_mixin_through_the_server() {
    let uri = "file:///extract_mixin.sass";
    let source = ".card\n   padding: 0\n   .title\n      margin: 0\n      color: red\n";

    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;
    open_document(&mut writer, uri, source).await;

    let range = serde_json::json!({
        "start": { "line": 3, "character": 6 },
        "end": { "line": 4, "character": 16 }
    });
    let edits = code_action_edits(
        &mut reader,
        &mut writer,
        uri,
        &range,
        &serde_json::json!([]),
        "Extract to mixin",
    )
    .await;

    let result = apply_lsp_edits(source, &edits);
    expect![[r"
        .card
           padding: 0
           @mixin new-mixin
              margin: 0
              color: red

           .title
              @include new-mixin
    "]]
    .assert_eq(&result);
    assert_parses_as_sass(&result);
}

#[tokio::test]
async fn sass_extract_variable_through_the_server() {
    let uri = "file:///extract_variable.sass";
    let source = ".card\n  .title\n    margin: 4px * 2\n";

    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;
    open_document(&mut writer, uri, source).await;

    let range = serde_json::json!({
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 19 }
    });
    let edits = code_action_edits(
        &mut reader,
        &mut writer,
        uri,
        &range,
        &serde_json::json!([]),
        "Extract to variable",
    )
    .await;

    let result = apply_lsp_edits(source, &edits);
    expect![[r"
        .card
          $new-variable: 4px * 2
          .title
            margin: $new-variable
    "]]
    .assert_eq(&result);
    assert_parses_as_sass(&result);
}
