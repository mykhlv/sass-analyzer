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
        "@use \"other.css\";\n",
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
