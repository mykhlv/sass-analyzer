use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use sass_parser::line_index::LineIndex;
use sass_parser::syntax::{GreenNode, SyntaxNode};
use sass_parser::text_range::TextRange;
use tokio::sync::mpsc;
use tower_lsp_server::Client;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Uri};

use crate::config::RuntimeConfig;
use crate::convert::text_range_to_lsp;
use crate::diagnostics;
use crate::symbols;
use crate::workspace;
use crate::{DocumentState, IncrementalEdit, Task};

#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub(crate) fn is_sass_file(uri: &Uri) -> bool {
    // Extract the path component before the query/fragment, then check extension.
    // Case-sensitive intentionally — matches filesystem behavior on Linux and
    // is consistent with collect_sass_recursive / check_workspace.
    let s = uri.as_str();
    let path_end = s.find(['?', '#']).unwrap_or(s.len());
    s[..path_end].ends_with(".sass")
}

pub(crate) fn parse_document(
    text: &str,
    sass: bool,
) -> Option<(GreenNode, Vec<(String, TextRange)>)> {
    let parse_fn = if sass {
        sass_parser::parse_sass
    } else {
        sass_parser::parse_scss
    };
    std::panic::catch_unwind(AssertUnwindSafe(|| parse_fn(text))).ok()
}

pub(crate) fn try_incremental_or_full(
    incremental: Option<IncrementalEdit>,
    text: &str,
    uri: &Uri,
) -> Option<(GreenNode, Vec<(String, TextRange)>)> {
    let sass = is_sass_file(uri);
    // Incremental reparse uses the SCSS lexer internally, so it's not
    // compatible with indented Sass files — always do a full reparse.
    if !sass && let Some(inc) = incremental {
        let result = sass_parser::reparse::incremental_reparse(
            &inc.old_green,
            &inc.old_errors,
            &inc.edit,
            text,
        );
        if let Some(result) = result {
            tracing::debug!(?uri, "incremental reparse");
            return Some(result);
        }
        tracing::debug!(?uri, "incremental reparse fell back");
    }
    let result = parse_document(text, sass);
    if result.is_none() {
        tracing::error!(?uri, "parser panic");
    }
    result
}

pub(crate) fn errors_to_diagnostics(
    errors: &[(String, TextRange)],
    line_index: &LineIndex,
    source: &str,
) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|(msg, range)| Diagnostic {
            range: text_range_to_lsp(*range, line_index, source),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("sass-analyzer".to_owned()),
            message: msg.clone(),
            ..Diagnostic::default()
        })
        .collect()
}

fn semantic_to_lsp(
    items: Vec<diagnostics::SemanticDiagnostic>,
    line_index: &LineIndex,
    source: &str,
) -> Vec<Diagnostic> {
    items
        .into_iter()
        .map(|d| Diagnostic {
            range: text_range_to_lsp(d.range, line_index, source),
            severity: Some(d.severity),
            source: Some("sass-analyzer".to_owned()),
            code: Some(tower_lsp_server::ls_types::NumberOrString::String(
                d.code.to_owned(),
            )),
            message: d.message,
            data: d.data,
            ..Diagnostic::default()
        })
        .collect()
}

/// Parse `text` as `version` of `uri`, index it and store it as the document's
/// state. Returns the diagnostics to publish, or `None` when the parse failed or
/// a newer version of the document is already stored.
fn apply_parse(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &workspace::ModuleGraph,
    uri: &Uri,
    version: i32,
    text: String,
    incremental: Option<IncrementalEdit>,
) -> Option<Vec<Diagnostic>> {
    if documents.get(uri).is_some_and(|s| s.version > version) {
        return None;
    }
    let (green, errors) = try_incremental_or_full(incremental, &text, uri)?;
    let line_index = LineIndex::new(&text);
    let mut all_diagnostics = errors_to_diagnostics(&errors, &line_index, &text);
    let file_symbols = {
        let root = SyntaxNode::new_root(green.clone());
        Arc::new(symbols::collect_symbols(&root))
    };

    module_graph.index_file(
        uri,
        green.clone(),
        file_symbols.clone(),
        line_index.clone(),
        text.clone(),
    );

    let semantic =
        diagnostics::check_file(uri, &file_symbols, module_graph, &green, !errors.is_empty());
    all_diagnostics.extend(semantic_to_lsp(semantic, &line_index, &text));

    let state = DocumentState {
        version,
        text,
        green,
        errors,
        line_index,
        symbols: file_symbols,
    };
    // Re-checked under the entry guard: only the worker indexes, but `did_open`
    // stores a document state of its own.
    let entry = documents.entry(uri.clone());
    if let Entry::Occupied(stored) = &entry
        && stored.get().version > version
    {
        return None;
    }
    entry.insert(state);
    Some(all_diagnostics)
}

/// Index the on-disk text of `uri` after a file-watcher event. Returns whether
/// the module graph changed.
///
/// The handler skips open files, but the file may have been opened since: the
/// editor's text then wins over the disk's.
fn apply_external_change(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &workspace::ModuleGraph,
    runtime_config: &RuntimeConfig,
    uri: &Uri,
    path: &Path,
) -> bool {
    if documents.contains_key(uri) {
        return false;
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("failed to read {}: {e}", path.display());
            return false;
        }
    };
    if text.len() > runtime_config.max_file_size() {
        return false;
    }
    let Some((green, _errors)) = parse_document(&text, is_sass_file(uri)) else {
        return false;
    };
    let line_index = LineIndex::new(&text);
    let file_symbols = {
        let root = SyntaxNode::new_root(green.clone());
        Arc::new(symbols::collect_symbols(&root))
    };
    module_graph.index_file(uri, green, file_symbols, line_index, text);
    true
}

/// Drop `uri` from the module graph after a file-watcher delete event, unless it
/// has been opened since the handler checked. Returns whether the module graph
/// changed.
fn apply_external_delete(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &workspace::ModuleGraph,
    uri: &Uri,
) -> bool {
    if documents.contains_key(uri) {
        return false;
    }
    module_graph.remove_file(uri);
    true
}

pub(crate) async fn run_worker(
    mut rx: mpsc::UnboundedReceiver<Task>,
    client: Client,
    documents: Arc<DashMap<Uri, DocumentState>>,
    module_graph: Arc<workspace::ModuleGraph>,
    runtime_config: Arc<RuntimeConfig>,
) {
    let mut pending: HashMap<Uri, (i32, String, Option<IncrementalEdit>)> = HashMap::new();
    let sleep = tokio::time::sleep(Duration::from_millis(runtime_config.debounce_ms()));
    tokio::pin!(sleep);
    let mut has_pending = false;

    loop {
        tokio::select! {
            task = rx.recv() => {
                let Some(task) = task else { break };
                match task {
                    Task::Parse { uri, version, text, incremental } => {
                        // If a previous edit is already pending, the old green is
                        // stale — discard incremental info and fall back to full parse.
                        let incremental = if pending.contains_key(&uri) {
                            None
                        } else {
                            incremental
                        };
                        pending.insert(uri, (version, text, incremental));
                        let debounce = Duration::from_millis(runtime_config.debounce_ms());
                        sleep.as_mut().reset(tokio::time::Instant::now() + debounce);
                        has_pending = true;
                    }
                    Task::Close { uri } => {
                        pending.remove(&uri);
                        documents.remove(&uri);
                        // Don't remove from module_graph: the file may still be a
                        // dependency of other files (indexed via index_dependency).
                        // VS Code sends didClose for peek previews after
                        // go-to-definition, which would destroy indexed dependencies.
                        client.publish_diagnostics(uri, vec![], None).await;
                    }
                    Task::ExternalChange { uri, path } => {
                        let is_new = !module_graph.contains(&uri);
                        if apply_external_change(
                            &documents, &module_graph, &runtime_config, &uri, &path,
                        ) {
                            // Importers of the changed file may now resolve its
                            // members differently. A new file may also be the one an
                            // unresolved import looked for; a change to a known one
                            // cannot make an import resolve.
                            let mut targets = module_graph.dependents_of(&uri);
                            if is_new {
                                targets.extend(documents_with_unresolved_imports(
                                    &documents, &module_graph, &[uri],
                                ));
                            }
                            refresh_documents(
                                &module_graph, &documents, &client, targets, is_new,
                            ).await;
                        }
                    }
                    Task::ExternalDelete { uri } => {
                        let targets = module_graph.dependents_of(&uri);
                        if apply_external_delete(&documents, &module_graph, &uri) {
                            refresh_documents(
                                &module_graph, &documents, &client, targets, true,
                            ).await;
                        }
                    }
                    Task::CheckWorkspace { root } => {
                        check_workspace(
                            &root, &client, &documents, &module_graph,
                            &runtime_config,
                        ).await;
                    }
                }
            }
            () = &mut sleep, if has_pending => {
                let mut new_uris = Vec::new();
                for (uri, (version, text, incremental)) in pending.drain() {
                    let is_new = !module_graph.contains(&uri);
                    if let Some(diagnostics) = apply_parse(
                        &documents, &module_graph, &uri, version, text, incremental,
                    ) {
                        if is_new {
                            new_uris.push(uri.clone());
                        }
                        client
                            .publish_diagnostics(uri, diagnostics, Some(version))
                            .await;
                    }
                }
                // A file opened in the editor before the watcher reports it never
                // reaches ExternalChange, so a new one is checked against the
                // unresolved imports here.
                if !new_uris.is_empty() {
                    let targets =
                        documents_with_unresolved_imports(&documents, &module_graph, &new_uris);
                    refresh_documents(&module_graph, &documents, &client, targets, true).await;
                }
                // Tell VS Code to re-request semantic tokens for all open editors.
                // Without this, tokens requested before parsing finishes get a null
                // response and are never refreshed.  Fire-and-forget so the
                // worker loop is never blocked by a slow or absent client.
                {
                    let c = client.clone();
                    tokio::spawn(async move { let _ = c.semantic_tokens_refresh().await; });
                }
                has_pending = false;
            }
        }
    }
}

/// Open documents, other than `except`, with an import that does not resolve.
fn documents_with_unresolved_imports(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &workspace::ModuleGraph,
    except: &[Uri],
) -> Vec<Uri> {
    documents
        .iter()
        .map(|entry| entry.key().clone())
        .filter(|uri| !except.contains(uri) && module_graph.has_unresolved_paths(uri))
        .collect()
}

/// Re-publish the diagnostics of the open documents among `targets`. With
/// `reindex`, each is indexed again first, so its imports resolve again against
/// the disk.
async fn refresh_documents(
    module_graph: &workspace::ModuleGraph,
    documents: &DashMap<Uri, DocumentState>,
    client: &Client,
    mut targets: Vec<Uri>,
    reindex: bool,
) {
    targets.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    targets.dedup();
    let mut published = false;
    for uri in targets {
        // Computed with the document guard released before publishing.
        let Some((all_diags, version)) = documents.get(&uri).map(|doc| {
            if reindex {
                module_graph.index_file(
                    &uri,
                    doc.green.clone(),
                    doc.symbols.clone(),
                    doc.line_index.clone(),
                    doc.text.clone(),
                );
            }
            (document_diagnostics(&uri, &doc, module_graph), doc.version)
        }) else {
            continue;
        };
        client
            .publish_diagnostics(uri, all_diags, Some(version))
            .await;
        published = true;
    }
    if published {
        let c = client.clone();
        tokio::spawn(async move {
            let _ = c.semantic_tokens_refresh().await;
        });
    }
}

/// Parse errors and semantic diagnostics of an open document.
fn document_diagnostics(
    uri: &Uri,
    doc: &DocumentState,
    module_graph: &workspace::ModuleGraph,
) -> Vec<Diagnostic> {
    let mut all_diags = errors_to_diagnostics(&doc.errors, &doc.line_index, &doc.text);
    let semantic = diagnostics::check_file(
        uri,
        &doc.symbols,
        module_graph,
        &doc.green,
        !doc.errors.is_empty(),
    );
    all_diags.extend(semantic_to_lsp(semantic, &doc.line_index, &doc.text));
    all_diags
}

// ── Check Workspace ─────────────────────────────────────────────────

struct ParsedFile {
    uri: Uri,
    green: GreenNode,
    errors: Vec<(String, TextRange)>,
    line_index: LineIndex,
    text: String,
    symbols: Arc<symbols::FileSymbols>,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
async fn check_workspace(
    root: &Path,
    client: &Client,
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &workspace::ModuleGraph,
    runtime_config: &RuntimeConfig,
) {
    let files = collect_sass_files(root);
    if files.is_empty() {
        () = client
            .show_message(
                tower_lsp_server::ls_types::MessageType::INFO,
                "sass-analyzer: no SCSS/Sass files found in workspace",
            )
            .await;
        return;
    }

    let total = files.len();
    let token = tower_lsp_server::ls_types::ProgressToken::String(
        "sass-analyzer/checkWorkspace".to_owned(),
    );
    let _ = client.create_work_done_progress(token.clone()).await;
    let progress = client
        .progress(token, "Checking workspace")
        .with_percentage(0)
        .with_message(format!("0/{total} files"))
        .begin()
        .await;

    let max_size = runtime_config.max_file_size();
    let mut total_diags = 0u64;
    let mut files_with_diags = 0u64;

    let mut parsed_files: Vec<ParsedFile> = Vec::with_capacity(files.len());

    // Phase 1: parse all files, index into module graph
    for (i, path) in files.iter().enumerate() {
        let uri = path_to_uri(path);

        // Skip files already open in editor (use their in-memory version)
        if documents.contains_key(&uri) {
            continue;
        }

        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if text.len() > max_size {
            continue;
        }

        let sass = path.extension().is_some_and(|ext| ext == "sass");
        let Some((green, errors)) = parse_document(&text, sass) else {
            continue;
        };
        let line_index = LineIndex::new(&text);
        let file_symbols = {
            let tree_root = SyntaxNode::new_root(green.clone());
            Arc::new(symbols::collect_symbols(&tree_root))
        };
        module_graph.index_file(
            &uri,
            green.clone(),
            file_symbols.clone(),
            line_index.clone(),
            text.clone(),
        );
        parsed_files.push(ParsedFile {
            uri,
            green,
            errors,
            line_index,
            text,
            symbols: file_symbols,
        });

        if (i + 1) % 100 == 0 {
            let pct = ((i + 1) as f64 / total as f64 * 50.0) as u32;
            progress
                .report_with_message(format!("Indexing {}/{total}", i + 1), pct)
                .await;
            tokio::task::yield_now().await;
        }
    }

    // Phase 2: run diagnostics on all parsed disk files
    let disk_total = parsed_files.len();
    for (i, pf) in parsed_files.iter().enumerate() {
        let mut all_diagnostics = errors_to_diagnostics(&pf.errors, &pf.line_index, &pf.text);
        let semantic = diagnostics::check_file(
            &pf.uri,
            &pf.symbols,
            module_graph,
            &pf.green,
            !pf.errors.is_empty(),
        );
        all_diagnostics.extend(semantic_to_lsp(semantic, &pf.line_index, &pf.text));

        if !all_diagnostics.is_empty() {
            total_diags += all_diagnostics.len() as u64;
            files_with_diags += 1;
        }
        client
            .publish_diagnostics(pf.uri.clone(), all_diagnostics, None)
            .await;

        if (i + 1) % 50 == 0 {
            let pct = (50.0 + (i + 1) as f64 / disk_total.max(1) as f64 * 40.0) as u32;
            progress
                .report_with_message(format!("Checking {}/{disk_total}", i + 1), pct)
                .await;
            tokio::task::yield_now().await;
        }
    }

    // Phase 3: re-check open files (they may now resolve more imports)
    // Collected first: a shard guard held across the publish below would block a
    // handler writing to that shard, which can keep the client channel from draining.
    let open_uris: Vec<Uri> = documents.iter().map(|entry| entry.key().clone()).collect();
    for uri in open_uris {
        let Some((all_diagnostics, version)) = documents
            .get(&uri)
            .map(|doc| (document_diagnostics(&uri, &doc, module_graph), doc.version))
        else {
            continue;
        };
        if !all_diagnostics.is_empty() {
            total_diags += all_diagnostics.len() as u64;
            files_with_diags += 1;
        }
        client
            .publish_diagnostics(uri, all_diagnostics, Some(version))
            .await;
    }

    progress
        .finish_with_message(format!(
            "Checked {total} files — {total_diags} diagnostics in {files_with_diags} files"
        ))
        .await;
}

fn path_to_uri(path: &Path) -> Uri {
    workspace::path_to_uri(path)
}

fn collect_sass_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_sass_recursive(root, &mut files);
    files.sort();
    files
}

fn collect_sass_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "node_modules" || name == "dist" || name == "build"
            {
                continue;
            }
            collect_sass_recursive(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "scss" || ext == "sass")
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_sass_file ────────────────────────────────────────────────

    #[test]
    fn is_sass_file_scss_extension() {
        let uri: Uri = "file:///project/styles.scss".parse().unwrap();
        assert!(!is_sass_file(&uri));
    }

    #[test]
    fn is_sass_file_sass_extension() {
        let uri: Uri = "file:///project/styles.sass".parse().unwrap();
        assert!(is_sass_file(&uri));
    }

    #[test]
    fn is_sass_file_css_extension() {
        let uri: Uri = "file:///project/styles.css".parse().unwrap();
        assert!(!is_sass_file(&uri));
    }

    #[test]
    fn is_sass_file_no_extension() {
        let uri: Uri = "file:///project/styles".parse().unwrap();
        assert!(!is_sass_file(&uri));
    }

    #[test]
    fn is_sass_file_with_query_params() {
        let uri: Uri = "file:///project/styles.sass?version=1".parse().unwrap();
        assert!(is_sass_file(&uri));
    }

    #[test]
    fn is_sass_file_partial_underscore() {
        let uri: Uri = "file:///project/_mixins.sass".parse().unwrap();
        assert!(is_sass_file(&uri));
    }

    // ── parse_document ──────────────────────────────────────────────

    #[test]
    fn parse_document_scss_valid() {
        let result = parse_document(".btn { color: red; }", false);
        assert!(result.is_some());
        let (green, errors) = result.unwrap();
        assert!(errors.is_empty());
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.text().to_string(), ".btn { color: red; }");
    }

    #[test]
    fn parse_document_scss_with_errors() {
        let result = parse_document("{{ invalid", false);
        assert!(result.is_some());
        let (green, errors) = result.unwrap();
        assert!(
            !errors.is_empty(),
            "malformed SCSS should produce parse errors"
        );
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.text().to_string(), "{{ invalid");
    }

    #[test]
    fn parse_document_sass_syntax() {
        let result = parse_document(".btn\n  color: red\n", true);
        assert!(result.is_some());
        let (green, errors) = result.unwrap();
        assert!(errors.is_empty(), "errors: {errors:?}");
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.text().to_string(), ".btn\n  color: red\n");
    }

    #[test]
    fn parse_document_empty_input() {
        let result = parse_document("", false);
        assert!(result.is_some());
        let (_green, errors) = result.unwrap();
        assert!(errors.is_empty());
    }

    // ── errors_to_diagnostics ───────────────────────────────────────

    #[test]
    fn errors_to_diagnostics_empty() {
        let li = sass_parser::line_index::LineIndex::new("");
        let diags = errors_to_diagnostics(&[], &li, "");
        assert!(diags.is_empty());
    }

    #[test]
    fn errors_to_diagnostics_single_error() {
        let src = ".btn { color: ; }";
        let li = sass_parser::line_index::LineIndex::new(src);
        let errors = vec![(
            "expected value".to_owned(),
            TextRange::new(14.into(), 14.into()),
        )];
        let diags = errors_to_diagnostics(&errors, &li, src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "expected value");
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[0].source.as_deref(), Some("sass-analyzer"));
    }

    #[test]
    fn errors_to_diagnostics_multiple_errors() {
        let src = ".btn { }";
        let li = sass_parser::line_index::LineIndex::new(src);
        let errors = vec![
            ("error one".to_owned(), TextRange::new(0.into(), 1.into())),
            ("error two".to_owned(), TextRange::new(5.into(), 6.into())),
        ];
        let diags = errors_to_diagnostics(&errors, &li, src);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].message, "error one");
        assert_eq!(diags[1].message, "error two");
    }

    // ── apply_parse ─────────────────────────────────────────────────

    fn defines(graph: &workspace::ModuleGraph, uri: &Uri, name: &str) -> bool {
        graph
            .get_symbols(uri)
            .is_some_and(|s| s.definitions.iter().any(|d| d.name == name))
    }

    #[test]
    fn stale_parse_changes_no_state() {
        let documents = DashMap::new();
        let graph = workspace::ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///a.scss".parse().unwrap();

        let published = apply_parse(&documents, &graph, &uri, 5, "$new: 1;\n".to_owned(), None);
        assert!(published.is_some());

        let published = apply_parse(&documents, &graph, &uri, 3, "$old: 1;\n".to_owned(), None);
        assert!(published.is_none(), "a stale parse must not be published");
        assert_eq!(documents.get(&uri).unwrap().text, "$new: 1;\n");
        assert!(
            defines(&graph, &uri, "new"),
            "the module graph lost the newer version"
        );
        assert!(
            !defines(&graph, &uri, "old"),
            "the module graph indexed a stale parse"
        );
    }

    #[test]
    fn same_version_parse_is_applied() {
        let documents = DashMap::new();
        let graph = workspace::ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///a.scss".parse().unwrap();

        apply_parse(&documents, &graph, &uri, 2, "$a: 1;\n".to_owned(), None);
        let published = apply_parse(&documents, &graph, &uri, 2, "$b: 1;\n".to_owned(), None);
        assert!(published.is_some());
        assert_eq!(documents.get(&uri).unwrap().text, "$b: 1;\n");
        assert!(defines(&graph, &uri, "b"));
    }

    // ── external changes ────────────────────────────────────────────

    #[test]
    fn external_change_of_an_open_document_keeps_the_editor_text() {
        let dir = std::env::temp_dir().join(format!("sass_ext_open_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.scss");
        std::fs::write(&path, "$disk: 1;\n").unwrap();
        let uri = workspace::path_to_uri(&path);
        let documents = DashMap::new();
        let config = RuntimeConfig::default();
        let graph = workspace::ModuleGraph::new(Arc::new(RuntimeConfig::default()));

        // The watcher event was checked against the open files before the file
        // was opened, and reaches the worker after the editor's text was indexed.
        apply_parse(
            &documents,
            &graph,
            &uri,
            1,
            "$editor: 1;\n".to_owned(),
            None,
        );
        let changed = apply_external_change(&documents, &graph, &config, &uri, &path);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(!changed);
        assert!(
            defines(&graph, &uri, "editor"),
            "disk text replaced the editor's"
        );
    }

    #[test]
    fn external_change_of_a_closed_file_is_indexed() {
        let dir = std::env::temp_dir().join(format!("sass_ext_closed_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.scss");
        std::fs::write(&path, "$disk: 1;\n").unwrap();
        let uri = workspace::path_to_uri(&path);
        let graph = workspace::ModuleGraph::new(Arc::new(RuntimeConfig::default()));

        let changed = apply_external_change(
            &DashMap::new(),
            &graph,
            &RuntimeConfig::default(),
            &uri,
            &path,
        );
        let _ = std::fs::remove_dir_all(&dir);

        assert!(changed);
        assert!(defines(&graph, &uri, "disk"));
    }

    #[test]
    fn external_delete_of_an_open_document_keeps_it_indexed() {
        let documents = DashMap::new();
        let graph = workspace::ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///a.scss".parse().unwrap();
        apply_parse(
            &documents,
            &graph,
            &uri,
            1,
            "$editor: 1;\n".to_owned(),
            None,
        );

        assert!(!apply_external_delete(&documents, &graph, &uri));
        assert!(defines(&graph, &uri, "editor"));
    }

    #[test]
    fn external_delete_of_a_closed_file_drops_it() {
        let graph = workspace::ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///a.scss".parse().unwrap();
        apply_parse(
            &DashMap::new(),
            &graph,
            &uri,
            1,
            "$a: 1;\n".to_owned(),
            None,
        );

        assert!(apply_external_delete(&DashMap::new(), &graph, &uri));
        assert!(!defines(&graph, &uri, "a"));
    }

    // ── try_incremental_or_full ─────────────────────────────────────

    #[test]
    fn try_incremental_or_full_no_incremental() {
        let uri: Uri = "file:///test.scss".parse().unwrap();
        let result = try_incremental_or_full(None, ".btn { color: red; }", &uri);
        assert!(result.is_some());
    }

    #[test]
    fn try_incremental_or_full_sass_file_always_full() {
        let uri: Uri = "file:///test.sass".parse().unwrap();
        let result = try_incremental_or_full(None, ".btn\n  color: red\n", &uri);
        assert!(result.is_some());
    }
}
