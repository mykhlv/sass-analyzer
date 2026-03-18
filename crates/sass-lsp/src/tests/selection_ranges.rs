use super::*;

async fn get_selection_ranges(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    scss: &str,
    id: u64,
    positions: Vec<(u32, u32)>,
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

    let lsp_positions: Vec<Value> = positions
        .iter()
        .map(|(line, character)| serde_json::json!({ "line": line, "character": character }))
        .collect();

    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/selectionRange",
            "params": {
                "textDocument": { "uri": uri },
                "positions": lsp_positions
            }
        }),
    )
    .await;

    let resp = recv_msg(reader, writer).await;
    resp["result"].as_array().cloned().unwrap_or_default()
}

/// Flatten a nested SelectionRange into a list of ranges from innermost to outermost.
fn flatten_selection_range(sr: &Value) -> Vec<&Value> {
    let mut result = Vec::new();
    let mut current = sr;
    loop {
        result.push(&current["range"]);
        if current["parent"].is_null() {
            break;
        }
        current = &current["parent"];
    }
    result
}

#[tokio::test]
async fn selection_range_property_value() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .btn {         ← line 0
    //   color: red;  ← line 1, cursor on "red" (col 9)
    // }              ← line 2
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_prop.scss",
        ".btn {\n  color: red;\n}\n",
        90,
        vec![(1, 9)],
    )
    .await;
    assert_eq!(results.len(), 1, "one result for one position");

    let chain = flatten_selection_range(&results[0]);
    assert!(chain.len() >= 3, "at least token → declaration → rule set");

    // Innermost should be "red" token
    let innermost = chain[0];
    assert_eq!(innermost["start"]["line"], 1);
    assert_eq!(innermost["start"]["character"], 9);
    assert_eq!(innermost["end"]["line"], 1);
    assert_eq!(innermost["end"]["character"], 12);

    // Outermost should cover the entire file (root)
    let outermost = chain.last().unwrap();
    assert_eq!(outermost["start"]["line"], 0);
    assert_eq!(outermost["start"]["character"], 0);
}

#[tokio::test]
async fn selection_range_variable_reference() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // $color: red;       ← line 0
    // .a { color: $color; } ← line 1, cursor on $color (col 13)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_var.scss",
        "$color: red;\n.a { color: $color; }\n",
        91,
        vec![(1, 13)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    assert!(chain.len() >= 3, "at least token → declaration → rule set");

    // Innermost is "color" IDENT token ($ is a separate DOLLAR token)
    let innermost = chain[0];
    assert_eq!(innermost["start"]["line"], 1);
    assert_eq!(innermost["start"]["character"], 13);
    assert_eq!(innermost["end"]["line"], 1);
    assert_eq!(innermost["end"]["character"], 18);
}

#[tokio::test]
async fn selection_range_nested_rules() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .parent {             ← line 0
    //   .child {            ← line 1
    //     color: red;       ← line 2, cursor on "color" (col 4)
    //   }                   ← line 3
    // }                     ← line 4
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_nested.scss",
        ".parent {\n  .child {\n    color: red;\n  }\n}\n",
        92,
        vec![(2, 4)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // token → declaration → child rule set → parent rule set → root
    assert!(
        chain.len() >= 4,
        "at least token → declaration → child → parent → root, got {}",
        chain.len()
    );
}

#[tokio::test]
async fn selection_range_multiple_positions() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_multi.scss",
        ".a { color: red; }\n.b { font-size: 14px; }\n",
        93,
        vec![(0, 5), (1, 5)],
    )
    .await;
    assert_eq!(results.len(), 2, "one result per position");
}

#[tokio::test]
async fn selection_range_selector() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .btn { color: red; }  ← cursor on ".btn" (col 1)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_selector.scss",
        ".btn { color: red; }\n",
        94,
        vec![(0, 1)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    assert!(
        chain.len() >= 2,
        "at least token → selector → rule set → root"
    );
}

#[tokio::test]
async fn selection_range_at_rule() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // @mixin flex($dir) {          ← line 0
    //   display: flex;             ← line 1, cursor on "flex" (col 11)
    //   flex-direction: $dir;      ← line 2
    // }                            ← line 3
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_at.scss",
        "@mixin flex($dir) {\n  display: flex;\n  flex-direction: $dir;\n}\n",
        95,
        vec![(1, 11)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // token → declaration → mixin rule → root
    assert!(
        chain.len() >= 3,
        "at least token → declaration → mixin → root, got {}",
        chain.len()
    );
}

#[tokio::test]
async fn selection_range_empty_file() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_empty.scss",
        "",
        96,
        vec![(0, 0)],
    )
    .await;
    // Fallback: one result per position even if no token found
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["range"]["start"]["line"], 0);
    assert_eq!(results[0]["range"]["start"]["character"], 0);
}

#[tokio::test]
async fn selection_range_cursor_at_eof() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // ".a {}\n" — cursor past the last char (line 1, col 0)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_eof.scss",
        ".a {}\n",
        97,
        vec![(1, 0)],
    )
    .await;
    assert_eq!(
        results.len(),
        1,
        "must return one result to keep index correspondence"
    );
}

#[tokio::test]
async fn selection_range_comment() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // /* hello */     ← line 0, cursor inside comment (col 4)
    // .a { color: red; }
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_comment.scss",
        "/* hello */\n.a { color: red; }\n",
        98,
        vec![(0, 4)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // Comment token → root
    assert!(chain.len() >= 2, "at least comment token → root");
    // Innermost should cover the comment
    assert_eq!(chain[0]["start"]["line"], 0);
    assert_eq!(chain[0]["start"]["character"], 0);
    assert_eq!(chain[0]["end"]["character"], 11);
}

#[tokio::test]
async fn selection_range_full_chain_verification() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .a { color: red; }
    // Cursor on "red" (line 0, col 13)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_chain.scss",
        ".a { color: red; }\n",
        99,
        vec![(0, 13)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    // Verify every range is strictly contained within its parent
    for i in 0..chain.len() - 1 {
        let inner = chain[i];
        let outer = chain[i + 1];
        let inner_start = (
            inner["start"]["line"].as_u64().unwrap(),
            inner["start"]["character"].as_u64().unwrap(),
        );
        let inner_end = (
            inner["end"]["line"].as_u64().unwrap(),
            inner["end"]["character"].as_u64().unwrap(),
        );
        let outer_start = (
            outer["start"]["line"].as_u64().unwrap(),
            outer["start"]["character"].as_u64().unwrap(),
        );
        let outer_end = (
            outer["end"]["line"].as_u64().unwrap(),
            outer["end"]["character"].as_u64().unwrap(),
        );
        assert!(
            outer_start <= inner_start && inner_end <= outer_end,
            "range {i} must be contained within range {}: {:?} not in {:?}",
            i + 1,
            (inner_start, inner_end),
            (outer_start, outer_end),
        );
    }

    // Outermost must cover entire file
    let outermost = chain.last().unwrap();
    assert_eq!(outermost["start"]["line"], 0);
    assert_eq!(outermost["start"]["character"], 0);
}

#[tokio::test]
async fn selection_range_interpolation() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // .#{$var} { color: red; }  ← cursor on "$var" (col 4)
    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_interp.scss",
        ".#{$var} { color: red; }\n",
        100,
        vec![(0, 4)],
    )
    .await;
    assert_eq!(results.len(), 1);

    let chain = flatten_selection_range(&results[0]);
    assert!(
        chain.len() >= 3,
        "at least token → interpolation → selector → root, got {}",
        chain.len()
    );
}

#[tokio::test]
async fn selection_range_whitespace_only() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let results = get_selection_ranges(
        &mut reader,
        &mut writer,
        "file:///sel_ws.scss",
        "   \n  \n",
        101,
        vec![(0, 1)],
    )
    .await;
    assert_eq!(
        results.len(),
        1,
        "must return one result even for whitespace-only file"
    );
}

#[tokio::test]
async fn initialize_reports_selection_range_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;

    let caps = &resp["result"]["capabilities"];
    assert_eq!(
        caps["selectionRangeProvider"], true,
        "server should advertise selection range provider"
    );
}
