use super::*;

#[tokio::test]
async fn completion_returns_local_symbols() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n@mixin btn { }\n@function double($n) { @return $n * 2; }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp.scss",
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
            "id": 20,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp.scss" },
                "position": { "line": 2, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(
        items.len(),
        3,
        "expected 3 completions: $color, btn, double"
    );

    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"$color"), "should contain $color");
    assert!(labels.contains(&"btn"), "should contain btn (mixin)");
    assert!(
        labels.contains(&"double"),
        "should contain double (function)"
    );
}

#[tokio::test]
async fn completion_returns_none_for_unknown_uri() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///unknown.scss" },
                "position": { "line": 0, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(
        resp["result"].is_null(),
        "completion for unknown file should be null"
    );
}

#[tokio::test]
async fn completion_item_kinds() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$v: 1;\n@mixin m { }\n@function f() { @return 1; }\n%ph { }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///kinds.scss",
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
            "id": 22,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///kinds.scss" },
                "position": { "line": 3, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 4, "expected 4 completions");

    let var = items.iter().find(|i| i["label"] == "$v").unwrap();
    assert_eq!(var["kind"], 6, "variable kind = 6");

    let mixin = items.iter().find(|i| i["label"] == "m").unwrap();
    assert_eq!(mixin["kind"], 2, "mixin kind = METHOD = 2");
    assert!(
        mixin["detail"].as_str().unwrap().contains("@mixin"),
        "mixin detail should contain @mixin"
    );

    let func = items.iter().find(|i| i["label"] == "f").unwrap();
    assert_eq!(func["kind"], 3, "function kind = 3");

    let placeholder = items.iter().find(|i| i["label"] == "%ph").unwrap();
    assert_eq!(placeholder["kind"], 7, "placeholder kind = CLASS = 7");
}

#[tokio::test]
async fn completion_after_dollar_only_variables() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss =
        "$color: red;\n@mixin btn { }\n@function double($n) { @return $n * 2; }\n.a { color: $";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_dollar.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "$" on line 3
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_dollar.scss" },
                "position": { "line": 3, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    // Should only contain variables, not mixins or functions
    for item in items {
        assert_eq!(
            item["kind"], 6,
            "after $ only variable items (kind=6), got: {}",
            item["label"]
        );
    }
    assert!(
        items.iter().any(|i| i["label"] == "$color"),
        "should contain $color"
    );
}

#[tokio::test]
async fn completion_after_include_only_mixins() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n@mixin btn { }\n@function double($n) { @return $n * 2; }\n.a {\n  @include \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_include.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on line 4: "  @include " (char 11 = end of "@include ")
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_include.scss" },
                "position": { "line": 4, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 1, "only mixins after @include");
    assert_eq!(items[0]["label"], "btn");
    assert_eq!(items[0]["kind"], 2, "mixin kind = METHOD = 2");
}

#[tokio::test]
async fn completion_sort_text_tiers() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$local: 1;\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_sort.scss",
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
            "id": 32,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_sort.scss" },
                "position": { "line": 0, "character": 0 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    // Local symbols should have sortText starting with "0_"
    let local = items.iter().find(|i| i["label"] == "$local").unwrap();
    assert!(
        local["sortText"].as_str().unwrap().starts_with("0_"),
        "local symbol sortText should start with 0_"
    );
}

#[tokio::test]
async fn completion_property_name_context() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  col\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_prop.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "col" at line 1, character 5
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 33,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_prop.scss" },
                "position": { "line": 1, "character": 5 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert!(!items.is_empty(), "should have CSS property completions");
    // All items should have kind = PROPERTY (10)
    for item in items {
        assert_eq!(item["kind"], 10, "property completion kind should be 10");
    }
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"color"), "should contain 'color'");
    assert!(
        labels.contains(&"column-count"),
        "should contain 'column-count'"
    );
}

#[tokio::test]
async fn completion_use_path_builtins() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@use \"sass:";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_use.scss",
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
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_use.scss" },
                "position": { "line": 0, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"sass:math"), "should contain sass:math");
    assert!(labels.contains(&"sass:color"), "should contain sass:color");
    assert!(labels.contains(&"sass:list"), "should contain sass:list");
}

#[tokio::test]
async fn completion_variable_shows_value_detail() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$primary: #3498db;\n.a { color: $";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///comp_detail.scss",
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
            "id": 35,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///comp_detail.scss" },
                "position": { "line": 1, "character": 14 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let primary = items.iter().find(|i| i["label"] == "$primary").unwrap();
    assert_eq!(
        primary["detail"].as_str().unwrap(),
        "#3498db",
        "variable detail should show its value"
    );
}

// ── CSS value completion tests ─────────────────────────────────────

#[tokio::test]
async fn completion_property_value_display() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  display: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_display.scss",
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
            "id": 36,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_display.scss" },
                "position": { "line": 1, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"flex"), "should contain 'flex'");
    assert!(labels.contains(&"grid"), "should contain 'grid'");
    assert!(labels.contains(&"block"), "should contain 'block'");
    assert!(labels.contains(&"none"), "should contain 'none'");
    assert!(
        labels.contains(&"inline-flex"),
        "should contain 'inline-flex'"
    );
    // Negative: position values should NOT appear in display completions
    assert!(
        !labels.contains(&"absolute"),
        "display should not contain 'absolute'"
    );
    assert!(
        !labels.contains(&"sticky"),
        "display should not contain 'sticky'"
    );
}

#[tokio::test]
async fn completion_property_value_position() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  position: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_pos.scss",
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
            "id": 37,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_pos.scss" },
                "position": { "line": 1, "character": 12 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"absolute"), "should contain 'absolute'");
    assert!(labels.contains(&"relative"), "should contain 'relative'");
    assert!(labels.contains(&"fixed"), "should contain 'fixed'");
    assert!(labels.contains(&"sticky"), "should contain 'sticky'");
    assert!(labels.contains(&"static"), "should contain 'static'");
    // Negative: display values should NOT appear in position completions
    assert!(
        !labels.contains(&"flex"),
        "position should not contain 'flex'"
    );
    assert!(
        !labels.contains(&"grid"),
        "position should not contain 'grid'"
    );
}

#[tokio::test]
async fn completion_property_value_with_partial() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  display: fl\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_partial.scss",
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
            "id": 38,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_partial.scss" },
                "position": { "line": 1, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"flex"), "should contain 'flex'");
    assert!(labels.contains(&"flow-root"), "should contain 'flow-root'");
    // "flex" should rank before "flow-root" (prefix match wins)
    let flex_idx = labels.iter().position(|l| *l == "flex").unwrap();
    let flow_idx = labels.iter().position(|l| *l == "flow-root").unwrap();
    assert!(flex_idx < flow_idx, "'flex' should rank before 'flow-root'");
}

#[tokio::test]
async fn completion_property_value_includes_variables() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$my-display: flex;\n.a {\n  display: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_vars.scss",
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
            "id": 39,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_vars.scss" },
                "position": { "line": 2, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    // Should have both CSS keyword values and Sass variables
    assert!(labels.contains(&"flex"), "should contain keyword 'flex'");
    assert!(
        labels.contains(&"$my-display"),
        "should contain variable '$my-display'"
    );
}

#[tokio::test]
async fn completion_property_value_unknown_property() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$x: 1;\n.a {\n  custom-prop: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_unknown.scss",
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
            "id": 40,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_unknown.scss" },
                "position": { "line": 2, "character": 15 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    // Unknown property still gets global keywords + Sass symbols
    assert!(labels.contains(&"inherit"), "should contain 'inherit'");
    assert!(labels.contains(&"initial"), "should contain 'initial'");
    assert!(labels.contains(&"$x"), "should contain variable '$x'");
}

#[tokio::test]
async fn completion_property_value_global_keywords() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  display: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_global.scss",
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
            "id": 41,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_global.scss" },
                "position": { "line": 1, "character": 11 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"inherit"), "should contain 'inherit'");
    assert!(labels.contains(&"initial"), "should contain 'initial'");
    assert!(labels.contains(&"unset"), "should contain 'unset'");
    assert!(labels.contains(&"revert"), "should contain 'revert'");
    assert!(
        labels.contains(&"revert-layer"),
        "should contain 'revert-layer'"
    );
}

#[tokio::test]
async fn completion_property_value_flex_direction() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  flex-direction: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_flexdir.scss",
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
            "id": 42,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_flexdir.scss" },
                "position": { "line": 1, "character": 18 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"row"), "should contain 'row'");
    assert!(labels.contains(&"column"), "should contain 'column'");
    assert!(
        labels.contains(&"row-reverse"),
        "should contain 'row-reverse'"
    );
    assert!(
        labels.contains(&"column-reverse"),
        "should contain 'column-reverse'"
    );
}

#[tokio::test]
async fn completion_property_value_enum_member_kind() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  position: \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///val_kind.scss",
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
            "id": 43,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///val_kind.scss" },
                "position": { "line": 1, "character": 12 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    // CSS value keywords should have kind = ENUM_MEMBER (20)
    let keyword_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| {
            let label = i["label"].as_str().unwrap_or("");
            !label.starts_with('$')
        })
        .collect();
    assert!(!keyword_items.is_empty());
    for item in keyword_items {
        assert_eq!(
            item["kind"], 20,
            "CSS value keyword should have kind ENUM_MEMBER (20), got {:?} for {:?}",
            item["kind"], item["label"]
        );
    }
}

#[tokio::test]
async fn completion_map_entry_not_property_value() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Map entry on its own line — should NOT get CSS value completions
    let scss = "$map: (\n  key: \n);\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///map_entry.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "key: " on line 1
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 60,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///map_entry.scss" },
                "position": { "line": 1, "character": 7 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    // Should NOT contain CSS value keywords like "flex", "grid", "none"
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(
            !labels.contains(&"flex"),
            "map entry should not offer CSS value 'flex', got: {labels:?}"
        );
        assert!(
            !labels.contains(&"grid"),
            "map entry should not offer CSS value 'grid', got: {labels:?}"
        );
    }
}

#[tokio::test]
async fn completion_multiline_value_context() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Multi-line declaration: value on a continuation line
    let scss = ".a {\n  display:\n    \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///multiline_val.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on line 2 (the continuation line after "display:\n")
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 61,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///multiline_val.scss" },
                "position": { "line": 2, "character": 4 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // Should offer display values since we're in value position
    assert!(
        labels.contains(&"flex"),
        "multi-line display value should offer 'flex', got: {labels:?}"
    );
    assert!(
        labels.contains(&"grid"),
        "multi-line display value should offer 'grid', got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_map_entry_with_css_property_key() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Map keys that happen to be valid CSS property names
    let scss = "$map: (\n  display: flex,\n  position: \n);\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///map_css_key.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "position: " on line 2 — inside a map, NOT a CSS declaration
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 62,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///map_css_key.scss" },
                "position": { "line": 2, "character": 12 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        // Should NOT offer CSS position values like "absolute", "fixed", "sticky"
        assert!(
            !labels.contains(&"absolute"),
            "map entry should not offer CSS value 'absolute', got: {labels:?}"
        );
        assert!(
            !labels.contains(&"sticky"),
            "map entry should not offer CSS value 'sticky', got: {labels:?}"
        );
    }
}

#[tokio::test]
async fn completion_multiline_value_with_partial() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Multi-line declaration with partial value text on continuation line
    let scss = ".a {\n  display:\n    fl\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///multiline_partial.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "fl" on line 2
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 63,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///multiline_partial.scss" },
                "position": { "line": 2, "character": 6 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions for multi-line partial");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // "flex" should match the "fl" prefix and be offered
    assert!(
        labels.contains(&"flex"),
        "multi-line partial 'fl' should offer 'flex', got: {labels:?}"
    );
    assert!(
        labels.contains(&"flow-root"),
        "multi-line partial 'fl' should offer 'flow-root', got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_multiline_value_multiple_declarations() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Multiple declarations; cursor on continuation line after the second one
    let scss = ".a {\n  color: red;\n  display:\n    \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///multi_decl.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on the blank continuation line (line 3) after "display:"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 64,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///multi_decl.scss" },
                "position": { "line": 3, "character": 4 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions for display after color");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // Should offer display values, not color values
    assert!(
        labels.contains(&"flex"),
        "should offer display values like 'flex', got: {labels:?}"
    );
    assert!(
        labels.contains(&"grid"),
        "should offer display values like 'grid', got: {labels:?}"
    );
    // Should NOT offer color-specific values (red is not a CSS keyword we enumerate)
    assert!(
        !labels.contains(&"absolute"),
        "should not offer position values, got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_nested_map_not_property_value() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Deeply nested map — inner key should not trigger CSS value completions
    let scss = "$theme: (\n  colors: (\n    primary: \n  )\n);\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///nested_map.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "primary: " on line 2
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 65,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///nested_map.scss" },
                "position": { "line": 2, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(
            !labels.contains(&"flex"),
            "nested map entry should not offer CSS value 'flex', got: {labels:?}"
        );
        assert!(
            !labels.contains(&"inherit"),
            "nested map entry should not offer global keyword 'inherit', got: {labels:?}"
        );
    }
}

#[tokio::test]
async fn completion_custom_property_multiline() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // Custom property with value on continuation line
    let scss = ".a {\n  --my-display:\n    \n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///custom_prop_ml.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on blank continuation line (line 2) after "--my-display:"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 66,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///custom_prop_ml.scss" },
                "position": { "line": 2, "character": 4 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"]
        .as_array()
        .expect("should return completions for custom property value");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    // Custom properties accept any value; we should at least get global keywords
    assert!(
        labels.contains(&"inherit"),
        "custom property value should offer 'inherit', got: {labels:?}"
    );
}

#[tokio::test]
async fn completion_decimal_not_namespace() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = ".a {\n  font-size: 1.\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///decimal.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "1." — should NOT treat "1" as namespace
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 67,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///decimal.scss" },
                "position": { "line": 1, "character": 15 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    // Should get PropertyValue completions (font-size keywords or global keywords),
    // not an empty namespace result
    if let Some(items) = resp["result"].as_array() {
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        // Should NOT be empty (which would happen if "1" was treated as a namespace)
        // PropertyValue for font-size doesn't have keyword values, but global keywords apply
        assert!(
            !labels.is_empty(),
            "decimal position should still offer completions, got empty"
        );
    }
}

#[tokio::test]
async fn completion_include_namespace_prefix() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // File with @use that creates a namespace, then @include with that namespace
    let scss = "@use \"sass:math\";\n.a {\n  @include math.\n}\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///include_ns.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor after "math." on line 2
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 68,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": "file:///include_ns.scss" },
                "position": { "line": 2, "character": 17 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    // With Namespace context, only symbols from math namespace should appear
    if let Some(items) = resp["result"].as_array() {
        for item in items {
            let label = item["label"].as_str().unwrap_or("");
            assert!(
                label.starts_with("math."),
                "all items should be from math namespace, got: {label}"
            );
        }
    }
}
