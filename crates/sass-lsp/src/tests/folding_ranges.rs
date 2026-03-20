use super::*;

// ---------------------------------------------------------------------------
// Folding ranges
// ---------------------------------------------------------------------------

async fn get_folds(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    scss: &str,
    id: u64,
) -> Vec<serde_json::Value> {
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
            "method": "textDocument/foldingRange",
            "params": {
                "textDocument": { "uri": uri }
            }
        }),
    )
    .await;

    let resp = recv_msg(reader, writer).await;
    resp["result"].as_array().unwrap().clone()
}

#[tokio::test]
async fn folding_range_rule_set() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .btn {      ← line 0
    //   color: red;   ← line 1
    //   font-size: 14px;  ← line 2
    // }            ← line 3  (closing brace stays visible)
    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_rule.scss",
        ".btn {\n  color: red;\n  font-size: 14px;\n}\n",
        60,
    )
    .await;
    assert_eq!(folds.len(), 1, "one fold for the rule set");
    assert_eq!(folds[0]["startLine"], 0);
    assert_eq!(folds[0]["endLine"], 2, "end_line excludes closing brace");
    assert!(folds[0]["kind"].is_null(), "block folds have no kind");
}

#[tokio::test]
async fn folding_range_nested_rules() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .parent {     ← line 0
    //   .child {    ← line 1
    //     color: red; ← line 2
    //   }           ← line 3
    // }             ← line 4
    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_nested.scss",
        ".parent {\n  .child {\n    color: red;\n  }\n}\n",
        61,
    )
    .await;
    assert_eq!(folds.len(), 2, "two folds: parent and child rule sets");
    // Parent fold: lines 0..3 (} on line 4 visible)
    assert_eq!(folds[0]["startLine"], 0);
    assert_eq!(folds[0]["endLine"], 3);
    // Child fold: lines 1..2 (} on line 3 visible)
    assert_eq!(folds[1]["startLine"], 1);
    assert_eq!(folds[1]["endLine"], 2);
}

#[tokio::test]
async fn folding_range_at_rules() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_at_rules.scss",
        "@mixin flex($dir) {\n  display: flex;\n  flex-direction: $dir;\n}\n\n\
         @media (min-width: 768px) {\n  .container {\n    width: 750px;\n  }\n}\n",
        62,
    )
    .await;
    // @mixin, @media, .container inside @media
    assert_eq!(
        folds.len(),
        3,
        "three folds: mixin, media, and inner rule set"
    );
}

#[tokio::test]
async fn folding_range_multiline_comment() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // /*          ← line 0
    //  * Multi    ← line 1
    //  * comment  ← line 2
    //  */         ← line 3
    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_comment.scss",
        "/*\n * Multi-line\n * comment\n */\n.btn { color: red; }\n",
        63,
    )
    .await;
    let comment_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("comment"))
        .collect();
    assert_eq!(comment_folds.len(), 1, "one comment fold");
    assert_eq!(comment_folds[0]["startLine"], 0);
    assert_eq!(comment_folds[0]["endLine"], 2, "end_line excludes */ line");
}

#[tokio::test]
async fn folding_range_region_markers() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_region.scss",
        "// #region Variables\n$color: red;\n$size: 14px;\n// #endregion\n",
        64,
    )
    .await;
    let region_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("region"))
        .collect();
    assert_eq!(region_folds.len(), 1, "one region fold");
    assert_eq!(region_folds[0]["startLine"], 0);
    assert_eq!(region_folds[0]["endLine"], 3);
}

#[tokio::test]
async fn folding_range_single_line_not_folded() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_single.scss",
        ".btn { color: red; }\n",
        65,
    )
    .await;
    assert_eq!(folds.len(), 0, "single-line rule should not produce a fold");
}

#[tokio::test]
async fn folding_range_consecutive_line_comments() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_consec.scss",
        "// First line\n// Second line\n// Third line\n.btn {\n  color: red;\n}\n",
        66,
    )
    .await;
    let comment_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("comment"))
        .collect();
    assert_eq!(comment_folds.len(), 1, "consecutive comments fold together");
    assert_eq!(comment_folds[0]["startLine"], 0);
    assert_eq!(comment_folds[0]["endLine"], 2);
}

#[tokio::test]
async fn folding_range_unmatched_region_ignored() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_unmatched.scss",
        "// #region No End\n$color: red;\n",
        67,
    )
    .await;
    let region_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("region"))
        .collect();
    assert_eq!(region_folds.len(), 0, "unmatched #region produces no fold");
}

#[tokio::test]
async fn folding_range_nested_regions() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(
        &mut reader,
        &mut writer,
        "file:///fold_nested_region.scss",
        "// #region Outer\n// #region Inner\n$x: 1;\n// #endregion\n$y: 2;\n// #endregion\n",
        68,
    )
    .await;
    let region_folds: Vec<_> = folds
        .iter()
        .filter(|f| f["kind"].as_str() == Some("region"))
        .collect();
    assert_eq!(region_folds.len(), 2, "nested regions produce two folds");
}

#[tokio::test]
async fn folding_range_empty_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let folds = get_folds(&mut reader, &mut writer, "file:///fold_empty.scss", "", 69).await;
    assert_eq!(folds.len(), 0, "empty file produces no folds");
}

#[tokio::test]
async fn initialize_reports_folding_range_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(
        caps["foldingRangeProvider"], true,
        "server should advertise folding range provider"
    );
}
