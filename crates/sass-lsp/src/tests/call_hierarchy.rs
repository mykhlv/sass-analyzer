use super::*;

// ── Call Hierarchy ──────────────────────────────────────────────────

#[tokio::test]
async fn call_hierarchy_prepare_on_function_definition() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2); }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_prep.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "add" in @function definition (line 0, char 10)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 80,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_prep.scss" },
                "position": { "line": 0, "character": 10 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "add");
    assert_eq!(items[0]["kind"], 12, "SymbolKind::FUNCTION = 12");
    assert!(items[0]["data"]["kind"] == "function");
}

#[tokio::test]
async fn call_hierarchy_prepare_on_mixin_definition() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss =
        "@mixin flex($dir) { display: flex; flex-direction: $dir; }\n.x { @include flex(row); }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_mixin.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "flex" in @mixin definition (line 0, char 7)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 81,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_mixin.scss" },
                "position": { "line": 0, "character": 7 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "flex");
    assert!(items[0]["data"]["kind"] == "mixin");
}

#[tokio::test]
async fn call_hierarchy_prepare_on_call_site() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1, 2); }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_call.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "add" in call site (line 1, char 12)
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 82,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_call.scss" },
                "position": { "line": 1, "character": 12 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "add");
}

#[tokio::test]
async fn call_hierarchy_prepare_on_variable_returns_null() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$color: red;\n.x { color: $color; }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_var.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on "$color" variable reference
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 83,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_var.scss" },
                "position": { "line": 1, "character": 13 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(resp["result"].is_null(), "variables are not callable");
}

#[tokio::test]
async fn call_hierarchy_incoming_calls() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
@function helper($x) { @return $x; }
@function caller_a($n) { @return helper($n); }
@function caller_b($n) { @return helper($n) + helper($n); }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_incoming.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // First prepare on "helper"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 84,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_incoming.scss" },
                "position": { "line": 0, "character": 10 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let item = items[0].clone();

    // Now get incoming calls
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 85,
            "method": "callHierarchy/incomingCalls",
            "params": { "item": item }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let calls = resp["result"].as_array().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "helper is called from caller_a and caller_b"
    );

    let caller_names: Vec<&str> = calls
        .iter()
        .map(|c| c["from"]["name"].as_str().unwrap())
        .collect();
    assert!(caller_names.contains(&"caller_a"));
    assert!(caller_names.contains(&"caller_b"));

    // caller_b calls helper twice
    let caller_b = calls
        .iter()
        .find(|c| c["from"]["name"] == "caller_b")
        .unwrap();
    assert_eq!(
        caller_b["fromRanges"].as_array().unwrap().len(),
        2,
        "caller_b calls helper twice"
    );
}

#[tokio::test]
async fn call_hierarchy_outgoing_calls() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
@function add($a, $b) { @return $a + $b; }
@function mul($a, $b) { @return $a * $b; }
@function compute($x) { @return add(mul($x, 2), 1); }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_outgoing.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Prepare on "compute"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 86,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_outgoing.scss" },
                "position": { "line": 2, "character": 10 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let item = items[0].clone();

    // Get outgoing calls
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 87,
            "method": "callHierarchy/outgoingCalls",
            "params": { "item": item }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let calls = resp["result"].as_array().unwrap();
    assert_eq!(calls.len(), 2, "compute calls add and mul");

    let callee_names: Vec<&str> = calls
        .iter()
        .map(|c| c["to"]["name"].as_str().unwrap())
        .collect();
    assert!(callee_names.contains(&"add"));
    assert!(callee_names.contains(&"mul"));
}

#[tokio::test]
async fn call_hierarchy_incoming_from_include() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
@mixin reset { margin: 0; }
.a { @include reset; }
.b { @include reset; }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_include.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Prepare on "reset" mixin
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 88,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_include.scss" },
                "position": { "line": 0, "character": 7 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let item = items[0].clone();
    assert!(item["data"]["kind"] == "mixin");

    // Get incoming calls
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 89,
            "method": "callHierarchy/incomingCalls",
            "params": { "item": item }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let calls = resp["result"].as_array().unwrap();
    // Both @includes are at top level (inside rule sets, not inside functions/mixins)
    // so they should be grouped as file-level callers
    assert!(
        !calls.is_empty(),
        "mixin should have at least 1 incoming call group"
    );
}

#[tokio::test]
async fn call_hierarchy_outgoing_from_mixin() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "\
@function double($n) { @return $n * 2; }
@mixin sized($w) { width: double($w); }
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_mixin_out.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Prepare on "sized" mixin
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 90,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_mixin_out.scss" },
                "position": { "line": 1, "character": 7 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let item = items[0].clone();

    // Get outgoing calls
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 91,
            "method": "callHierarchy/outgoingCalls",
            "params": { "item": item }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let calls = resp["result"].as_array().unwrap();
    assert_eq!(calls.len(), 1, "sized calls double");
    assert_eq!(calls[0]["to"]["name"], "double");
}

#[tokio::test]
async fn call_hierarchy_nested_callable_not_attributed_to_outer() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    // inner() calls helper(), but outer() should NOT list helper() as outgoing
    let scss = "\
@function helper() { @return 1; }
@function outer() {
  @function inner() { @return helper(); }
  @return inner();
}
";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_nested.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Prepare on "outer"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 92,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_nested.scss" },
                "position": { "line": 1, "character": 10 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let item = items[0].clone();
    assert_eq!(item["name"], "outer");

    // Get outgoing calls from outer
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 93,
            "method": "callHierarchy/outgoingCalls",
            "params": { "item": item }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let calls = resp["result"].as_array().unwrap();
    let callee_names: Vec<&str> = calls
        .iter()
        .map(|c| c["to"]["name"].as_str().unwrap())
        .collect();
    assert!(callee_names.contains(&"inner"), "outer calls inner");
    assert!(
        !callee_names.contains(&"helper"),
        "helper is called by inner, not outer"
    );
}

#[tokio::test]
async fn call_hierarchy_cross_file_incoming() {
    // Create temp directory BEFORE initialize so we can pass it as rootUri.
    let dir = std::env::temp_dir().join(format!("sass_ch_cross_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;
    let helpers_path = dir.join("_helpers.scss");
    let main_path = dir.join("main.scss");

    let helpers_scss = "@function double($x) { @return $x * 2; }\n";
    let main_scss = "@use 'helpers';\n.a { width: helpers.double(10px); }\n";

    std::fs::write(&helpers_path, helpers_scss).unwrap();
    std::fs::write(&main_path, main_scss).unwrap();

    let helpers_uri = file_uri(&helpers_path);
    let main_uri = file_uri(&main_path);

    // Open both files.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": helpers_uri,
                    "languageId": "scss",
                    "version": 1,
                    "text": helpers_scss
                }
            }
        }),
    )
    .await;
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "scss",
                    "version": 1,
                    "text": main_scss
                }
            }
        }),
    )
    .await;

    // Retry prepareCallHierarchy until the server has indexed the file.
    // tower-lsp-server processes didOpen concurrently, so the file may not
    // be ready on the first attempt.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut id = 100u64;
    let item = loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "prepareCallHierarchy never returned a result for helpers"
        );
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "textDocument/prepareCallHierarchy",
                "params": {
                    "textDocument": { "uri": helpers_uri },
                    "position": { "line": 0, "character": 10 }
                }
            }),
        )
        .await;

        let resp = recv_response(&mut reader, &mut writer, id).await;
        id += 1;
        if let Some(items) = resp["result"].as_array() {
            if !items.is_empty() {
                assert_eq!(items[0]["name"], "double");
                break items[0].clone();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };

    // Incoming calls — retry until the cross-file reference is indexed.
    let calls = loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "incomingCalls never returned results"
        );
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "callHierarchy/incomingCalls",
                "params": { "item": item }
            }),
        )
        .await;

        let resp = recv_response(&mut reader, &mut writer, id).await;
        id += 1;
        if let Some(arr) = resp["result"].as_array() {
            if !arr.is_empty() {
                break arr.clone();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };

    assert_eq!(
        calls.len(),
        1,
        "double is called from one location (main.scss top-level)"
    );
    let from = &calls[0]["from"];
    assert_eq!(from["name"], "main.scss");

    // Clean up
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn call_hierarchy_cross_file_outgoing() {
    // Create temp directory BEFORE initialize so we can pass it as rootUri.
    let dir = std::env::temp_dir().join(format!("sass_ch_cross_out_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;
    let helpers_path = dir.join("_helpers.scss");
    let main_path = dir.join("main.scss");

    let helpers_scss = "@function double($x) { @return $x * 2; }\n";
    let main_scss = "@use 'helpers';\n@function quadruple($x) { @return helpers.double(helpers.double($x)); }\n";

    std::fs::write(&helpers_path, helpers_scss).unwrap();
    std::fs::write(&main_path, main_scss).unwrap();

    let helpers_uri = file_uri(&helpers_path);
    let main_uri = file_uri(&main_path);

    // Open both files.
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": helpers_uri,
                    "languageId": "scss",
                    "version": 1,
                    "text": helpers_scss
                }
            }
        }),
    )
    .await;
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "scss",
                    "version": 1,
                    "text": main_scss
                }
            }
        }),
    )
    .await;

    // Retry prepareCallHierarchy until the server has indexed the file.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut id = 200u64;
    let item = loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "prepareCallHierarchy never returned a result for main"
        );
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "textDocument/prepareCallHierarchy",
                "params": {
                    "textDocument": { "uri": main_uri },
                    "position": { "line": 1, "character": 10 }
                }
            }),
        )
        .await;

        let resp = recv_response(&mut reader, &mut writer, id).await;
        id += 1;
        if let Some(items) = resp["result"].as_array() {
            if !items.is_empty() {
                assert_eq!(items[0]["name"], "quadruple");
                break items[0].clone();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };

    // Outgoing calls — retry until cross-file resolution is ready.
    let calls = loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "outgoingCalls never returned results"
        );
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "callHierarchy/outgoingCalls",
                "params": { "item": item }
            }),
        )
        .await;

        let resp = recv_response(&mut reader, &mut writer, id).await;
        id += 1;
        if let Some(arr) = resp["result"].as_array() {
            if !arr.is_empty() {
                break arr.clone();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };

    assert_eq!(calls.len(), 1, "quadruple calls one unique target (double)");
    assert_eq!(calls[0]["to"]["name"], "double");
    assert_eq!(
        calls[0]["fromRanges"].as_array().unwrap().len(),
        2,
        "double is called twice from quadruple"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn call_hierarchy_recursive_function() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@function fact($n) { @return if($n <= 1, 1, $n * fact($n - 1)); }\n";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ch_recursive.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Prepare on "fact"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 94,
            "method": "textDocument/prepareCallHierarchy",
            "params": {
                "textDocument": { "uri": "file:///ch_recursive.scss" },
                "position": { "line": 0, "character": 10 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let items = resp["result"].as_array().unwrap();
    let item = items[0].clone();

    // Outgoing: fact calls itself
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 95,
            "method": "callHierarchy/outgoingCalls",
            "params": { "item": item.clone() }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let calls = resp["result"].as_array().unwrap();
    assert!(
        calls.iter().any(|c| c["to"]["name"] == "fact"),
        "fact calls itself"
    );

    // Incoming: fact is called by itself
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 96,
            "method": "callHierarchy/incomingCalls",
            "params": { "item": item }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let calls = resp["result"].as_array().unwrap();
    assert!(
        calls.iter().any(|c| c["from"]["name"] == "fact"),
        "fact is called by itself"
    );
}
