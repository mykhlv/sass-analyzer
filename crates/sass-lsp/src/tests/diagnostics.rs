use super::*;

// ── Semantic diagnostics tests ─────────────────────────────────────

/// Open a document and return the published diagnostics.
async fn open_and_get_diagnostics(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    text: &str,
    version: i32,
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
                    "version": version,
                    "text": text
                }
            }
        }),
    )
    .await;
    let notif = recv_msg(reader, writer).await;
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    notif["params"]["diagnostics"].as_array().unwrap().clone()
}

// ── Arg count tests ────────────────────────────────────────────────

#[tokio::test]
async fn semantic_too_few_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args1.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "should report too few args");
    assert!(
        semantic[0]["message"]
            .as_str()
            .unwrap()
            .contains("at least 2"),
    );
}

#[tokio::test]
async fn semantic_exact_args_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args2.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert!(
        semantic.is_empty(),
        "exact args should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_too_many_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2, 3); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args3.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "should report too many args");
    assert!(
        semantic[0]["message"]
            .as_str()
            .unwrap()
            .contains("at most 2"),
    );
}

#[tokio::test]
async fn semantic_args_with_defaults_ok() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f($a, $b: 10px) { @return $a + $b; }\n.x { width: f(1); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args4.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert!(
        semantic.is_empty(),
        "call with default-covered args should be ok"
    );
}

#[tokio::test]
async fn semantic_args_with_rest_param() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f($a, $rest...) { @return $a; }\n.x { width: f(1, 2, 3, 4); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args5.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert!(semantic.is_empty(), "rest param should accept any count");
}

#[tokio::test]
async fn semantic_mixin_too_few_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@mixin flex($dir, $wrap) { display: flex; }\n.x { @include flex(); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args6.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "mixin with too few args should error");
}

#[tokio::test]
async fn semantic_zero_param_called_with_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f() { @return 1; }\n.x { width: f(42); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///args7.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("wrong-arg-count"))
        .collect();
    assert_eq!(semantic.len(), 1, "zero-param function called with args");
}

// ── Undefined reference tests ──────────────────────────────────────

#[tokio::test]
async fn semantic_undefined_variable() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { color: $undefined-var; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef1.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert_eq!(semantic.len(), 1);
    assert!(
        semantic[0]["message"]
            .as_str()
            .unwrap()
            .contains("undefined-var"),
    );
    assert_eq!(semantic[0]["severity"], 2, "should be WARNING (2)");
}

#[tokio::test]
async fn semantic_defined_variable_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "$color: red;\n.x { color: $color; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef2.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "defined variable should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_local_var_in_mixin_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "\
@mixin btnVariant($variant) {
  $common: red;
  background-color: $common;
  &-disabled {
    $disabled: blue;
    background-color: $disabled;
    color: $common;
  }
}";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///local_var.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "local variables in mixin body should not be flagged as undefined, got: {semantic:?}"
    );
}

#[tokio::test]
async fn semantic_top_level_var_used_in_mixin_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@use 'sass:map';\n\
$other-var: red;\n\
$btn-size: (lg: 16px, sm: 13px);\n\
@mixin btnSize($size) {\n\
  font-size: map.get($btn-size, $size);\n\
}";
    let diags = open_and_get_diagnostics(
        &mut reader,
        &mut writer,
        "file:///top_level_var.scss",
        text,
        1,
    )
    .await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c == "undefined-variable")
                && d["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("btn-size"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "top-level $btn-size should not be flagged as undefined, got: {semantic:?}"
    );
}

#[tokio::test]
async fn semantic_css_var_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { color: var(--custom); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef3.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        semantic.is_empty(),
        "CSS var() should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_css_calc_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { width: calc(100% - 20px); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef4.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        semantic.is_empty(),
        "CSS calc() should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_undefined_function() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { width: nonexistent-fn(1); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef5.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert_eq!(semantic.len(), 1);
}

#[tokio::test]
async fn semantic_undefined_mixin() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { @include nonexistent-mixin(); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef6.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-mixin"))
        .collect();
    assert_eq!(semantic.len(), 1);
}

#[tokio::test]
async fn semantic_defined_function_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function double($n) { @return $n * 2; }\n.x { width: double(5); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef7.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "defined function should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_defined_mixin_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@mixin bold { font-weight: bold; }\n.x { @include bold; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef8.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "defined mixin should produce no diagnostic"
    );
}

#[tokio::test]
async fn semantic_placeholder_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { @extend %placeholder; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///undef9.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("undefined"))
        })
        .collect();
    assert!(
        semantic.is_empty(),
        "placeholder @extend should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_diagnostics_have_codes() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function f($a) { @return $a; }\n.x { width: f(); color: $nope; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///codes.scss", text, 1).await;

    let with_codes: Vec<_> = diags.iter().filter(|d| d["code"].is_string()).collect();
    assert!(
        with_codes.len() >= 2,
        "should have at least arg-count + undefined diagnostics with codes"
    );
}

#[tokio::test]
async fn semantic_import_suppresses_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@import 'variables';\n.x { color: $imported-var; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///suppress1.scss", text, 1).await;

    let semantic: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert!(
        semantic.is_empty(),
        "files with @import should suppress undefined warnings"
    );
}

#[tokio::test]
async fn semantic_function_param_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@function clamp-val($min, $max, $val) { @return max($min, min($max, $val)); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///param1.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert!(
        undef.is_empty(),
        "function parameters should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_mixin_param_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@mixin flex($dir, $wrap: nowrap) { flex-direction: $dir; flex-wrap: $wrap; }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///param2.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-variable"))
        .collect();
    assert!(
        undef.is_empty(),
        "mixin parameters should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_each_loop_var_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "$list: a, b, c;\n@each $item in $list { .#{$item} { display: block; } }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///loop1.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"].as_str() == Some("undefined-variable")
                && d["message"].as_str().is_some_and(|m| m.contains("item"))
        })
        .collect();
    assert!(
        undef.is_empty(),
        "@each loop variable should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_for_loop_var_not_undefined() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = "@for $i from 1 through 3 { .col-#{$i} { width: percentage($i / 12); } }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///loop2.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["code"].as_str() == Some("undefined-variable")
                && d["message"].as_str().is_some_and(|m| m.contains("`i`"))
        })
        .collect();
    assert!(
        undef.is_empty(),
        "@for loop variable should not be flagged as undefined"
    );
}

#[tokio::test]
async fn semantic_gradient_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { background: linear-gradient(to right, red, blue); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///css1.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        undef.is_empty(),
        "CSS linear-gradient() should not trigger undefined"
    );
}

#[tokio::test]
async fn semantic_transform_no_diagnostic() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let text = ".x { transform: translateX(10px) rotate(45deg) scale(1.5); }";
    let diags =
        open_and_get_diagnostics(&mut reader, &mut writer, "file:///css2.scss", text, 1).await;

    let undef: Vec<_> = diags
        .iter()
        .filter(|d| d["code"].as_str() == Some("undefined-function"))
        .collect();
    assert!(
        undef.is_empty(),
        "CSS transform functions should not trigger undefined"
    );
}
