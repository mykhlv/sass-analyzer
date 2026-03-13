use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use sass_parser::line_index::LineIndex;
use sass_parser::syntax::SyntaxNode;
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

pub(crate) fn parse_document(text: &str) -> Option<(rowan::GreenNode, Vec<(String, TextRange)>)> {
    std::panic::catch_unwind(AssertUnwindSafe(|| sass_parser::parse(text))).ok()
}

pub(crate) fn try_incremental_or_full(
    incremental: Option<IncrementalEdit>,
    text: &str,
    uri: &Uri,
) -> Option<(rowan::GreenNode, Vec<(String, TextRange)>)> {
    if let Some(inc) = incremental {
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
    let result = parse_document(text);
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
            ..Diagnostic::default()
        })
        .collect()
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
                    Task::ExternalChange { uri, text } => {
                        let Some((green, _errors)) = parse_document(&text) else {
                            continue;
                        };
                        let line_index = LineIndex::new(&text);
                        let file_symbols = {
                            let root = SyntaxNode::new_root(green.clone());
                            Arc::new(symbols::collect_symbols(&root))
                        };

                        module_graph.index_file(
                            &uri,
                            green,
                            file_symbols,
                            line_index,
                            text,
                        );

                        // Re-publish diagnostics for open files that import
                        // the changed file, since their cross-file references
                        // may now resolve differently.
                        refresh_dependents(
                            &module_graph, &documents, &client, &uri,
                        ).await;
                    }
                    Task::ExternalDelete { uri } => {
                        module_graph.remove_file(&uri);
                        refresh_dependents(
                            &module_graph, &documents, &client, &uri,
                        ).await;
                    }
                }
            }
            () = &mut sleep, if has_pending => {
                for (uri, (version, text, incremental)) in pending.drain() {
                    let Some((green, errors)) = try_incremental_or_full(
                        incremental, &text, &uri,
                    ) else {
                        continue;
                    };
                    let line_index = LineIndex::new(&text);
                    let mut all_diagnostics =
                        errors_to_diagnostics(&errors, &line_index, &text);
                    let file_symbols = {
                        let root = SyntaxNode::new_root(green.clone());
                        Arc::new(symbols::collect_symbols(&root))
                    };

                    let is_current = documents
                        .get(&uri)
                        .is_none_or(|state| state.version <= version);

                    module_graph.index_file(
                        &uri,
                        green.clone(),
                        file_symbols.clone(),
                        line_index.clone(),
                        text.clone(),
                    );

                    let semantic = diagnostics::check_file(
                        &uri, &file_symbols, &module_graph, &green,
                    );
                    all_diagnostics.extend(semantic_to_lsp(
                        semantic, &line_index, &text,
                    ));

                    documents.insert(
                        uri.clone(),
                        DocumentState {
                            version,
                            text,
                            green,
                            errors,
                            line_index,
                            symbols: file_symbols,
                        },
                    );

                    if is_current {
                        client
                            .publish_diagnostics(uri, all_diagnostics, Some(version))
                            .await;
                    }
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

/// Re-publish diagnostics for open files that depend on a changed/deleted file.
async fn refresh_dependents(
    module_graph: &workspace::ModuleGraph,
    documents: &DashMap<Uri, DocumentState>,
    client: &Client,
    changed_uri: &Uri,
) {
    for dep_uri in module_graph.dependents_of(changed_uri) {
        if let Some(doc) = documents.get(&dep_uri) {
            let mut all_diags = errors_to_diagnostics(&doc.errors, &doc.line_index, &doc.text);
            let semantic =
                diagnostics::check_file(&dep_uri, &doc.symbols, module_graph, &doc.green);
            all_diags.extend(semantic_to_lsp(semantic, &doc.line_index, &doc.text));
            client
                .publish_diagnostics(dep_uri.clone(), all_diags, Some(doc.version))
                .await;
        }
    }
    let c = client.clone();
    tokio::spawn(async move {
        let _ = c.semantic_tokens_refresh().await;
    });
}
