use super::*;

// ── Document link tests ─────────────────────────────────────────

#[tokio::test]
async fn document_link_for_use_rule() {
    let dir = std::env::temp_dir().join(format!("sass_doclink_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;

    let colors_path = dir.join("_colors.scss");
    let main_path = dir.join("main.scss");
    std::fs::write(&colors_path, "$c: red;\n").unwrap();
    std::fs::write(&main_path, "@use 'colors';\n").unwrap();

    let main_uri = file_uri(&main_path);

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
                    "text": "@use 'colors';\n"
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
            "id": 90,
            "method": "textDocument/documentLink",
            "params": {
                "textDocument": { "uri": main_uri }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let links = resp["result"].as_array().unwrap();
    assert_eq!(links.len(), 1, "expected 1 document link for @use");
    assert!(links[0]["target"].as_str().unwrap().contains("colors"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn goto_definition_on_use_path() {
    let dir = std::env::temp_dir().join(format!("sass_gotoimp_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;

    let helpers_path = dir.join("_helpers.scss");
    let main_path = dir.join("main.scss");
    std::fs::write(&helpers_path, "$x: 1;\n").unwrap();
    let main_scss = "@use 'helpers';\n";
    std::fs::write(&main_path, main_scss).unwrap();

    let main_uri = file_uri(&main_path);

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
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Cursor on 'helpers' string — char 5 is inside the quoted string
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 91,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 0, "character": 6 }
            }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = &resp["result"];
    assert!(
        result["uri"].as_str().unwrap().contains("helpers"),
        "goto-def on @use path should jump to the file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn rename_updates_forward_show_clause() {
    let dir = std::env::temp_dir().join(format!("sass_renfwd_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir)).await;

    let colors_path = dir.join("_colors.scss");
    let index_path = dir.join("_index.scss");
    let main_path = dir.join("main.scss");

    let colors_scss = "$primary: red;\n";
    let index_scss = "@forward 'colors' show $primary;\n";
    let main_scss = "@use 'index';\n.a { color: index.$primary; }\n";

    std::fs::write(&colors_path, colors_scss).unwrap();
    std::fs::write(&index_path, index_scss).unwrap();
    std::fs::write(&main_path, main_scss).unwrap();

    let colors_uri = file_uri(&colors_path);
    let index_uri = file_uri(&index_path);
    let main_uri = file_uri(&main_path);

    // Open all files
    for (uri, text) in [
        (&colors_uri, colors_scss),
        (&index_uri, index_scss),
        (&main_uri, main_scss),
    ] {
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": "scss",
                        "version": 1,
                        "text": text
                    }
                }
            }),
        )
        .await;
    }

    // Retry rename until server has indexed all files.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut id = 92u64;
    let resp = loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "rename never returned a result with @forward edits"
        );
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": colors_uri },
                    "position": { "line": 0, "character": 1 },
                    "newName": "brand"
                }
            }),
        )
        .await;
        id += 1;

        let r = recv_msg(&mut reader, &mut writer).await;
        if r["error"].is_null() && !r["result"].is_null() {
            let changes = &r["result"]["changes"];
            if !changes[&index_uri].is_null() {
                break r;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    let changes = &resp["result"]["changes"];
    let index_edits = changes[&index_uri].as_array().unwrap();
    assert!(
        !index_edits.is_empty(),
        "@forward show clause should be updated on rename"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
