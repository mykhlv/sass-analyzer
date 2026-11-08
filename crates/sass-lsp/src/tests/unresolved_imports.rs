use super::*;

const NOT_FOUND: &str = "Can't find stylesheet to import.";

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("sass_unresolved_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, rel: &str, text: &str) -> std::path::PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The next diagnostics published for `uri`, or a panic after five seconds.
async fn next_diagnostics(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
) -> Vec<Value> {
    let wait = async {
        loop {
            let msg = recv_msg(reader, writer).await;
            if msg["method"] == "textDocument/publishDiagnostics" && msg["params"]["uri"] == uri {
                return msg["params"]["diagnostics"].as_array().unwrap().clone();
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), wait)
        .await
        .unwrap_or_else(|_| panic!("no diagnostics published for {uri}"))
}

/// Open `text` as `rel` in a server rooted at `dir`; return its diagnostics.
async fn open_in(
    dir: &TempDir,
    rel: &str,
    text: &str,
) -> (
    tokio::io::DuplexStream,
    tokio::io::DuplexStream,
    String,
    Vec<Value>,
) {
    let uri = file_uri(&dir.write(rel, text));
    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir.0)).await;
    open_document(&mut writer, &uri, text).await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    (reader, writer, uri, diags)
}

fn not_found(diags: &[Value]) -> Vec<&Value> {
    diags
        .iter()
        .filter(|d| d["code"] == "unresolved-import")
        .collect()
}

fn range_of(d: &Value) -> (u64, u64, u64, u64) {
    let r = &d["range"];
    (
        r["start"]["line"].as_u64().unwrap(),
        r["start"]["character"].as_u64().unwrap(),
        r["end"]["line"].as_u64().unwrap(),
        r["end"]["character"].as_u64().unwrap(),
    )
}

#[tokio::test]
async fn misspelled_use_is_one_error_on_its_url() {
    let dir = TempDir::new("use_typo");
    dir.write("_colors.scss", "$x: red;\n");
    let text = "@use \"colrs\";\na { b: colrs.$x; c: $y; d: f(); }\n";
    let (_r, _w, _uri, diags) = open_in(&dir, "main.scss", text).await;

    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(diags[0]["code"], "unresolved-import");
    assert_eq!(diags[0]["message"], NOT_FOUND);
    assert_eq!(diags[0]["severity"], 1);
    assert_eq!(range_of(&diags[0]), (0, 5, 0, 12));
}

#[tokio::test]
async fn misspelled_forward_is_an_error_on_its_url() {
    let dir = TempDir::new("forward_typo");
    let (_r, _w, _uri, diags) =
        open_in(&dir, "main.scss", "$a: 1;\n@forward 'nope' show $x;\n").await;

    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(range_of(&diags[0]), (1, 9, 1, 15));
}

#[tokio::test]
async fn missing_sass_import_is_an_error_on_its_url() {
    let dir = TempDir::new("import_missing");
    dir.write("_found.scss", "$x: 1;\n");
    let (_r, _w, _uri, diags) =
        open_in(&dir, "main.scss", "@import \"found\", \"missing\";\n").await;

    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(diags[0]["message"], NOT_FOUND);
    assert_eq!(range_of(&diags[0]), (0, 17, 0, 26));
}

#[tokio::test]
async fn unknown_builtin_module_is_an_error() {
    let dir = TempDir::new("builtin_unknown");
    let (_r, _w, _uri, diags) = open_in(&dir, "main.scss", "@use \"sass:nope\";\n").await;

    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");
}

#[tokio::test]
async fn builtin_scheme_and_plain_css_urls_are_not_errors() {
    let dir = TempDir::new("not_flagged");
    let text = concat!(
        "@use \"sass:math\";\n",
        "@use \"pkg:bootstrap\";\n",
        "@import \"a.css\";\n",
        "@import url(b);\n",
        "@import url(\"c\");\n",
        "@import \"http://x/d\";\n",
        "@import \"//x/e\";\n",
        "@import \"f\" screen;\n",
        "@import \"~pkg/g\";\n",
        "a { b: $undefined; }\n",
    );
    let (_r, _w, _uri, diags) = open_in(&dir, "main.scss", text).await;

    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn unresolved_import_in_indented_sass() {
    let dir = TempDir::new("sass_syntax");
    let text = "@use \"nope\"\n@import \"m\" screen\n";
    let (_r, _w, _uri, diags) = open_in(&dir, "main.sass", text).await;

    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(range_of(&diags[0]), (0, 5, 0, 11));
}

#[tokio::test]
async fn load_paths_change_clears_the_error() {
    let dir = TempDir::new("load_paths");
    dir.write("vendor/scss/_tokens.scss", "$gap: 4px;\n");
    let text = "@use \"tokens\";\na { b: tokens.$gap; }\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeConfiguration",
            "params": { "settings": { "sass-analyzer": { "loadPaths": ["vendor/scss"] } } }
        }),
    )
    .await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn import_aliases_change_clears_the_error() {
    let dir = TempDir::new("aliases");
    dir.write("src/styles/_tokens.scss", "$gap: 4px;\n");
    let text = "@use \"@styles/tokens\";\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeConfiguration",
            "params": {
                "settings": { "sass-analyzer": { "importAliases": { "@styles": "src/styles" } } }
            }
        }),
    )
    .await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    assert!(diags.is_empty(), "{diags:#?}");
}

async fn send_watched(writer: &mut (impl AsyncWriteExt + Unpin), uri: &str, typ: u32) {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": { "changes": [{ "uri": uri, "type": typ }] }
        }),
    )
    .await;
}

#[tokio::test]
async fn creating_the_missing_file_clears_the_error() {
    let dir = TempDir::new("created");
    let text = "@use \"later\";\na { b: later.$x; }\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");

    let created = file_uri(&dir.write("_later.scss", "$x: 1;\n"));
    send_watched(&mut writer, &created, 1).await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn deleting_an_imported_file_reports_the_error() {
    let dir = TempDir::new("deleted");
    let dep = dir.write("_dep.scss", "$x: 1;\n");
    let text = "@use \"dep\";\na { b: dep.$x; }\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert!(diags.is_empty(), "{diags:#?}");

    std::fs::remove_file(&dep).unwrap();
    send_watched(&mut writer, &file_uri(&dep), 3).await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(diags[0]["code"], "unresolved-import");
}

// ── Files Dart Sass and bundlers load ───────────────────────────────

async fn definition_of(
    reader: &mut (impl AsyncReadExt + Unpin),
    writer: &mut (impl AsyncWriteExt + Unpin),
    uri: &str,
    line: u32,
    character: u32,
) -> Value {
    send_msg(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 900, "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        }),
    )
    .await;
    recv_response(reader, writer, 900).await["result"].clone()
}

#[tokio::test]
async fn css_files_and_encoded_urls_resolve() {
    let dir = TempDir::new("css_files");
    dir.write(
        "node_modules/normalize.css/normalize.css",
        "html { margin: 0; }\n",
    );
    dir.write("vendor.css", "a { b: c; }\n");
    dir.write("theme/_index.css", "a { b: c; }\n");
    dir.write("my file.scss", "$x: 1;\n");
    let text = concat!(
        "@import \"normalize.css/normalize\";\n",
        "@use \"vendor\";\n",
        "@use \"theme\";\n",
        "@use \"my%20file\" as m;\n",
    );
    let (_r, _w, _uri, diags) = open_in(&dir, "main.scss", text).await;
    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn import_only_file_resolves_for_import() {
    let dir = TempDir::new("import_only");
    let target = dir.write("_bar.import.scss", "$x: 1;\n");
    dir.write("_bar.scss", "$y: 1;\n");
    let text = "@import \"bar\";\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert!(diags.is_empty(), "{diags:#?}");
    let def = definition_of(&mut reader, &mut writer, &uri, 0, 10).await;
    assert_eq!(def["uri"], file_uri(&target), "{def}");
}

#[tokio::test]
async fn import_only_file_does_not_resolve_for_use() {
    let dir = TempDir::new("import_only_use");
    dir.write("_bar.import.scss", "$x: 1;\n");
    let (_r, _w, _uri, diags) = open_in(&dir, "main.scss", "@use \"bar\";\n").await;
    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");
}

#[tokio::test]
async fn package_entry_from_package_json() {
    let dir = TempDir::new("package_json");
    dir.write(
        "node_modules/bootstrap/package.json",
        r#"{ "sass": "scss/bootstrap.scss" }"#,
    );
    let target = dir.write("node_modules/bootstrap/scss/bootstrap.scss", "$x: 1;\n");
    dir.write(
        "node_modules/@acme/ui/package.json",
        r#"{ "exports": { ".": { "sass": "./src/_ui.scss", "default": "./index.js" } } }"#,
    );
    dir.write("node_modules/@acme/ui/src/_ui.scss", "$y: 1;\n");
    dir.write(
        "node_modules/plain/package.json",
        r#"{ "style": "dist/plain.css" }"#,
    );
    dir.write("node_modules/plain/dist/plain.css", "a { b: c; }\n");
    let text = "@use \"bootstrap\";\n@use \"@acme/ui\";\n@use \"plain\";\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert!(diags.is_empty(), "{diags:#?}");
    let def = definition_of(&mut reader, &mut writer, &uri, 0, 7).await;
    assert_eq!(def["uri"], file_uri(&target), "{def}");
}

#[tokio::test]
async fn installed_package_is_not_reported_missing() {
    // A bundler may resolve what the server cannot, as a subpath in `exports`.
    let dir = TempDir::new("installed_package");
    dir.write(
        "node_modules/widgets/package.json",
        r#"{ "main": "index.js" }"#,
    );
    dir.write("node_modules/@scope/kit/README.md", "kit\n");
    let text = "@use \"widgets\";\n@use \"widgets/theme\";\n@use \"@scope/kit/x\";\n";
    let (_r, _w, _uri, diags) = open_in(&dir, "main.scss", text).await;
    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn opening_a_new_file_clears_the_error() {
    // An editor's "New File" opens the file before the watcher reports it, and the
    // watcher event of an open file is skipped.
    let dir = TempDir::new("opened_new");
    let text = "@use \"later\";\na { b: later.$x; }\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");

    let created = file_uri(&dir.write("_later.scss", ""));
    open_document(&mut writer, &created, "").await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn change_of_a_known_file_does_not_recheck_unrelated_documents() {
    let dir = TempDir::new("known_change");
    let other = dir.write("_other.scss", "$x: 1;\n");
    let other_uri = file_uri(&other);
    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir.0)).await;
    // Opened first, so that opening it re-checks nothing.
    let probe = file_uri(&dir.write("probe.scss", "$p: 1;\n"));
    open_document(&mut writer, &probe, "$p: 1;\n").await;
    next_diagnostics(&mut reader, &mut writer, &probe).await;
    let main = "@use \"pkg:theme\";\n";
    let uri = file_uri(&dir.write("main.scss", main));
    open_document(&mut writer, &uri, main).await;
    next_diagnostics(&mut reader, &mut writer, &uri).await;

    // The first event brings `_other.scss` into the module graph; a later one
    // changes a file already known, which cannot make an import resolve.
    send_watched(&mut writer, &other_uri, 2).await;
    next_diagnostics(&mut reader, &mut writer, &uri).await;
    std::fs::write(&other, "$x: 2;\n").unwrap();
    send_watched(&mut writer, &other_uri, 2).await;
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": probe, "version": 2 },
                "contentChanges": [{ "text": "$p: 2;\n" }]
            }
        }),
    )
    .await;
    // The worker handles the watcher event before the debounced parse.
    let republished = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let msg = recv_msg(&mut reader, &mut writer).await;
            if msg["method"] == "textDocument/publishDiagnostics" {
                if msg["params"]["uri"] == uri {
                    return true;
                }
                if msg["params"]["uri"] == probe {
                    return false;
                }
            }
        }
    })
    .await
    .expect("the probe's diagnostics");
    assert!(!republished, "an unrelated document was re-checked");
}

#[tokio::test]
async fn unresolved_imports_can_be_left_unreported() {
    let dir = TempDir::new("report_off");
    let text = "@use \"colrs\";\na { b: colrs.$x; c: $y; }\n";
    let uri = file_uri(&dir.write("main.scss", text));
    let (mut reader, mut writer) = spawn_server();
    do_initialize_with(
        &mut reader,
        &mut writer,
        serde_json::json!({ "reportUnresolvedImports": false }),
    )
    .await;
    open_document(&mut writer, &uri, text).await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    // The undefined names stay suppressed: their cause is the import.
    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn turning_the_report_off_clears_the_error() {
    let dir = TempDir::new("report_toggle");
    let (mut reader, mut writer, uri, diags) =
        open_in(&dir, "main.scss", "@use \"colrs\";\n").await;
    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeConfiguration",
            "params": { "settings": { "sass-analyzer": { "reportUnresolvedImports": false } } }
        }),
    )
    .await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    assert!(diags.is_empty(), "{diags:#?}");
}

#[tokio::test]
async fn css_url_resolves_to_its_file() {
    let dir = TempDir::new("css_url");
    let reset = dir.write("reset.css", "a { b: c; }\n");
    let theme = dir.write("_theme.css", "a { b: c; }\n");
    let text = "@use \"reset.css\";\n@forward \"theme.css\";\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    assert!(diags.is_empty(), "{diags:#?}");
    let def = definition_of(&mut reader, &mut writer, &uri, 0, 8).await;
    assert_eq!(def["uri"], file_uri(&reset), "{def}");
    let def = definition_of(&mut reader, &mut writer, &uri, 1, 12).await;
    assert_eq!(def["uri"], file_uri(&theme), "{def}");
}

/// Dart Sass loads the file a `@use` or `@forward` of a `.css` URL names, and fails
/// when there is none; an `@import` of one is left to the browser. `x.CSS` is not a
/// CSS URL to Dart Sass, so an `@import` of it is looked up and reported too.
#[tokio::test]
async fn missing_css_file_is_an_error_for_use_and_forward_only() {
    let dir = TempDir::new("css_missing");
    let text = concat!(
        "@use \"nope.css\";\n",
        "@forward \"gone.css\";\n",
        "@import \"plain.css\";\n",
        "@import \"upper.CSS\";\n",
    );
    let (_r, _w, _uri, diags) = open_in(&dir, "main.scss", text).await;
    let ranges: Vec<_> = not_found(&diags).into_iter().map(range_of).collect();
    assert_eq!(
        ranges,
        [(0, 5, 0, 15), (1, 9, 1, 19), (3, 8, 3, 19)],
        "{diags:#?}"
    );
}

#[tokio::test]
async fn url_with_trailing_slash_loads_the_directory_index() {
    let dir = TempDir::new("trailing_slash");
    dir.write("d.scss", "$x: 1;\n");
    let index = dir.write("d/_index.scss", "$x: 2;\n");
    dir.write("q.scss", "$x: 3;\n");
    let text = "@use \"d/\" as d;\n@use \"q/\" as q;\n";
    let (mut reader, mut writer, uri, diags) = open_in(&dir, "main.scss", text).await;
    let ranges: Vec<_> = not_found(&diags).into_iter().map(range_of).collect();
    assert_eq!(ranges, [(1, 5, 1, 9)], "{diags:#?}");
    let def = definition_of(&mut reader, &mut writer, &uri, 0, 7).await;
    assert_eq!(def["uri"], file_uri(&index), "{def}");
}

/// Many files appearing at once, as on a `git checkout`, re-check an open document
/// whose import does not resolve once, not once per file.
#[tokio::test]
async fn a_burst_of_new_files_rechecks_a_document_once() {
    const NEW_FILES: usize = 5;
    let dir = TempDir::new("burst");
    let (mut reader, mut writer) = spawn_server();
    do_initialize_with_root(&mut reader, &mut writer, &file_uri(&dir.0)).await;
    let probe = file_uri(&dir.write("probe.scss", "$p: 1;\n"));
    open_document(&mut writer, &probe, "$p: 1;\n").await;
    next_diagnostics(&mut reader, &mut writer, &probe).await;
    let main = "@use \"later\";\n";
    let uri = file_uri(&dir.write("main.scss", main));
    open_document(&mut writer, &uri, main).await;
    let diags = next_diagnostics(&mut reader, &mut writer, &uri).await;
    assert_eq!(not_found(&diags).len(), 1, "{diags:#?}");

    let changes: Vec<Value> = (0..NEW_FILES)
        .map(|i| {
            let created = dir.write(&format!("_new{i}.scss"), "$x: 1;\n");
            serde_json::json!({ "uri": file_uri(&created), "type": 1 })
        })
        .collect();
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": { "changes": changes }
        }),
    )
    .await;
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": probe, "version": 2 },
                "contentChanges": [{ "text": "$p: 2;\n" }]
            }
        }),
    )
    .await;
    // The worker handles the watcher events before the debounced parse.
    let publishes = tokio::time::timeout(Duration::from_secs(5), async {
        let mut publishes = 0;
        loop {
            let msg = recv_msg(&mut reader, &mut writer).await;
            if msg["method"] == "textDocument/publishDiagnostics" {
                if msg["params"]["uri"] == uri {
                    publishes += 1;
                } else if msg["params"]["uri"] == probe {
                    return publishes;
                }
            }
        }
    })
    .await
    .expect("the probe's diagnostics");
    assert_eq!(publishes, 1, "the burst should re-check it once");
}
