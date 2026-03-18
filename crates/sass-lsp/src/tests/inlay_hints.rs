use super::*;

// ── Inlay Hints ────────────────────────────────────────────────────

async fn get_inlay_hints(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    scss: &str,
    id: u64,
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
            "method": "textDocument/inlayHint",
            "params": {
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 100, "character": 0 }
                }
            }
        }),
    )
    .await;

    let resp = recv_msg(reader, writer).await;
    let result = &resp["result"];
    if result.is_null() {
        return Vec::new();
    }
    result.as_array().cloned().unwrap_or_default()
}

#[tokio::test]
async fn inlay_hint_function_call_positional_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1px, 2px); }";
    let hints = get_inlay_hints(&mut reader, &mut writer, "file:///hint_func.scss", scss, 70).await;

    assert_eq!(hints.len(), 2, "two positional args → two hints");
    assert_eq!(hints[0]["label"], "$a:");
    assert_eq!(hints[1]["label"], "$b:");
    assert_eq!(hints[0]["kind"], 2, "InlayHintKind::PARAMETER = 2");
}

#[tokio::test]
async fn inlay_hint_mixin_include_positional_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss =
        "@mixin flex($direction, $wrap) { display: flex; }\n.x { @include flex(row, nowrap); }";
    let hints = get_inlay_hints(
        &mut reader,
        &mut writer,
        "file:///hint_mixin.scss",
        scss,
        71,
    )
    .await;

    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0]["label"], "$direction:");
    assert_eq!(hints[1]["label"], "$wrap:");
}

#[tokio::test]
async fn inlay_hint_keyword_args_skipped() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1px, $b: 2px); }";
    let hints = get_inlay_hints(&mut reader, &mut writer, "file:///hint_kw.scss", scss, 72).await;

    assert_eq!(hints.len(), 1, "only positional arg gets hint");
    assert_eq!(hints[0]["label"], "$a:");
}

#[tokio::test]
async fn inlay_hint_single_param_skipped() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function double($n) { @return $n * 2; }\n.x { width: double(5px); }";
    let hints = get_inlay_hints(
        &mut reader,
        &mut writer,
        "file:///hint_single.scss",
        scss,
        73,
    )
    .await;

    assert!(hints.is_empty(), "single-param calls should not get hints");
}

#[tokio::test]
async fn inlay_hint_no_args() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin reset() { margin: 0; }\n.x { @include reset(); }";
    let hints = get_inlay_hints(
        &mut reader,
        &mut writer,
        "file:///hint_noargs.scss",
        scss,
        74,
    )
    .await;

    assert!(hints.is_empty(), "no-arg calls should return no hints");
}

#[tokio::test]
async fn inlay_hint_rest_param_stops() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin shadow($x, $rest...) { box-shadow: $x $rest; }\n\
                .x { @include shadow(2px, 3px, red); }";
    let hints = get_inlay_hints(&mut reader, &mut writer, "file:///hint_rest.scss", scss, 75).await;

    assert_eq!(hints.len(), 1, "only $x gets a hint, $rest... stops hints");
    assert_eq!(hints[0]["label"], "$x:");
}

#[tokio::test]
async fn inlay_hint_unresolved_call_no_hints() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".x { width: unknown-fn(1, 2, 3); }";
    let hints = get_inlay_hints(
        &mut reader,
        &mut writer,
        "file:///hint_unresolved.scss",
        scss,
        76,
    )
    .await;

    assert!(hints.is_empty(), "unresolved call should return no hints");
}

#[tokio::test]
async fn inlay_hint_padding_right_set() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2); }";
    let hints = get_inlay_hints(
        &mut reader,
        &mut writer,
        "file:///hint_padding.scss",
        scss,
        77,
    )
    .await;

    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0]["paddingRight"], true, "space after colon");
    assert_eq!(hints[0]["paddingLeft"], false);
}

#[tokio::test]
async fn inlay_hint_extra_args_beyond_params() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2, 3); }";
    let hints = get_inlay_hints(
        &mut reader,
        &mut writer,
        "file:///hint_extra.scss",
        scss,
        78,
    )
    .await;

    assert_eq!(
        hints.len(),
        2,
        "only $a and $b get hints, extra arg ignored"
    );
    assert_eq!(hints[0]["label"], "$a:");
    assert_eq!(hints[1]["label"], "$b:");
}

#[tokio::test]
async fn inlay_hint_keyword_arg_breaks_positional() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Keyword arg in the middle — all args after it should be keyword too (Sass rule)
    let scss = "@function f($a, $b, $c) { @return $a; }\n.x { width: f(1, $b: 2, $c: 3); }";
    let hints = get_inlay_hints(
        &mut reader,
        &mut writer,
        "file:///hint_kwbreak.scss",
        scss,
        79,
    )
    .await;

    assert_eq!(hints.len(), 1, "only first positional arg gets hint");
    assert_eq!(hints[0]["label"], "$a:");
}
