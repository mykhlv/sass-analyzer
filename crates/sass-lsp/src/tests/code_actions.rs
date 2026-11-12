//! Code actions through the server, on indented-Sass (`.sass`) documents.
//!
//! The server picks the statement syntax from the document's extension, so these
//! go through `textDocument/codeAction` rather than calling the actions directly.

use std::fmt::Write as _;

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

/// The line breaks other than `\n` that the code actions keep: `\r\n` and a lone `\r`.
const LINE_BREAKS: [&str; 2] = ["\r\n", "\r"];

/// Assert that every line break in `text` is `eol`.
fn assert_only_line_break(text: &str, eol: &str) {
    let lf = text.matches('\n').count();
    let crlf = text.matches("\r\n").count();
    let cr = text.matches('\r').count();
    let ok = if eol == "\r\n" {
        lf == crlf && cr == crlf
    } else {
        lf == 0
    };
    assert!(ok, "line breaks other than {eol:?} in {text:?}");
}

/// The edits of the `title_prefix` action on `source` (a file named `name` in a
/// fresh workspace that also holds `others`), applied.
async fn apply_action_on_disk(
    name: &str,
    source: &str,
    others: &[(&str, &str)],
    range: &Value,
    diagnostics: &Value,
    title_prefix: &str,
) -> String {
    let dir = std::env::temp_dir().join(format!(
        "sass_ca_{}_{}",
        std::process::id(),
        name.replace('.', "_")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut other_uris = Vec::new();
    for (other, text) in others {
        let path = dir.join(other);
        std::fs::write(&path, text).unwrap();
        other_uris.push((file_uri(&path), *text));
    }
    let path = dir.join(name);
    std::fs::write(&path, source).unwrap();
    let uri = file_uri(&path);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;
    for (other_uri, text) in &other_uris {
        open_document(&mut writer, other_uri, text).await;
    }
    open_document(&mut writer, &uri, source).await;
    let edits = code_action_edits(
        &mut reader,
        &mut writer,
        &uri,
        range,
        diagnostics,
        title_prefix,
    )
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    apply_lsp_edits(source, &edits)
}

fn undefined_primary(range: &Value) -> Value {
    serde_json::json!([{
        "range": range,
        "message": "undefined variable `primary`",
        "code": "undefined-variable",
        "data": { "name": "primary", "kind": "variable" }
    }])
}

/// Auto-import writes the `@use` with the document's own line ending, `\r\n` or a lone
/// `\r`, both after an existing import and at the top of a file with none.
#[tokio::test]
async fn auto_import_keeps_crlf_and_cr_line_endings() {
    let cases = [
        (
            "after_use.scss",
            "@use \"sass:math\";\n.a {\n  color: $primary;\n}\n",
            "_colors.scss",
            "$primary: red;\n",
            2,
        ),
        (
            "no_use.scss",
            ".a {\n  color: $primary;\n}\n",
            "_colors.scss",
            "$primary: red;\n",
            1,
        ),
        (
            "after_use.sass",
            "@use \"sass:math\"\n.a\n  color: $primary\n",
            "_colors.sass",
            "$primary: red\n",
            2,
        ),
        (
            "no_use.sass",
            ".a\n  color: $primary\n",
            "_colors.sass",
            "$primary: red\n",
            1,
        ),
    ];
    for eol in LINE_BREAKS {
        let mut results = String::new();
        for (name, source, other, other_text, line) in cases {
            let range = serde_json::json!({
                "start": { "line": line, "character": 9 },
                "end": { "line": line, "character": 17 }
            });
            let other_text = other_text.replace('\n', eol);
            let result = apply_action_on_disk(
                name,
                &source.replace('\n', eol),
                &[(other, &other_text)],
                &range,
                &undefined_primary(&range),
                "Add `@use",
            )
            .await;
            assert_only_line_break(&result, eol);
            let _ = writeln!(results, "{name}:\n{}", result.replace(eol, "\n"));
        }
        expect![[r#"
            after_use.scss:
            @use "sass:math";
            @use "colors";
            .a {
              color: colors.$primary;
            }

            no_use.scss:
            @use "colors";
            .a {
              color: colors.$primary;
            }

            after_use.sass:
            @use "sass:math"
            @use "colors"
            .a
              color: colors.$primary

            no_use.sass:
            @use "colors"
            .a
              color: colors.$primary

        "#]]
        .assert_eq(&results);
    }
}

/// Extract to variable writes the declaration with the document's own line
/// ending, `\r\n` or a lone `\r`, in both syntaxes.
#[tokio::test]
async fn extract_variable_keeps_crlf_and_cr_line_endings() {
    let range = serde_json::json!({
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 19 }
    });
    for eol in LINE_BREAKS {
        let mut results = String::new();
        for (name, source) in [
            (
                "var.scss",
                ".card {\n  .title {\n    margin: 4px * 2;\n  }\n}\n",
            ),
            ("var.sass", ".card\n  .title\n    margin: 4px * 2\n"),
        ] {
            let result = apply_action_on_disk(
                name,
                &source.replace('\n', eol),
                &[],
                &range,
                &serde_json::json!([]),
                "Extract to variable",
            )
            .await;
            assert_only_line_break(&result, eol);
            let _ = writeln!(results, "{name}:\n{}", result.replace(eol, "\n"));
        }
        expect![[r#"
            var.scss:
            .card {
              $new-variable: 4px * 2;
              .title {
                margin: $new-variable;
              }
            }

            var.sass:
            .card
              $new-variable: 4px * 2
              .title
                margin: $new-variable

        "#]]
        .assert_eq(&results);
    }
}

/// Extract to mixin writes the mixin, its body lines and the blank line after it
/// with the document's own line ending, `\r\n` or a lone `\r`, in both syntaxes.
#[tokio::test]
async fn extract_mixin_keeps_crlf_and_cr_line_endings() {
    for eol in LINE_BREAKS {
        let mut results = String::new();
        for (name, source, end) in [
            (
                "mixin.scss",
                ".card {\n  .title {\n    margin: 0;\n    color: red;\n  }\n}\n",
                15,
            ),
            (
                "mixin.sass",
                ".card\n  .title\n    margin: 0\n    color: red\n",
                14,
            ),
        ] {
            let range = serde_json::json!({
                "start": { "line": 2, "character": 4 },
                "end": { "line": 3, "character": end }
            });
            let result = apply_action_on_disk(
                name,
                &source.replace('\n', eol),
                &[],
                &range,
                &serde_json::json!([]),
                "Extract to mixin",
            )
            .await;
            assert_only_line_break(&result, eol);
            let _ = writeln!(results, "{name}:\n{}", result.replace(eol, "\n"));
        }
        expect![[r#"
            mixin.scss:
            .card {
              @mixin new-mixin {
                margin: 0;
                color: red;
              }

              .title {
                @include new-mixin;
              }
            }

            mixin.sass:
            .card
              @mixin new-mixin
                margin: 0
                color: red

              .title
                @include new-mixin

        "#]]
        .assert_eq(&results);
    }
}
