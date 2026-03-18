mod ast_helpers;
mod builtins;
mod call_hierarchy;
mod code_actions;
mod completion;
mod config;
mod convert;
mod css_properties;
mod css_values;
mod diagnostics;
mod folding;
mod highlights;
mod hover;
mod inlay_hints;
mod navigation;
mod sassdoc;
mod selection;
mod semantic_tokens;
mod signature_help;
mod symbols;
mod worker;
mod workspace;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use sass_parser::syntax::{GreenNode, SyntaxNode};
use sass_parser::text_range::{TextRange, TextSize};
use tokio::sync::mpsc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CallHierarchyServerCapability, CodeActionKind, CodeActionOptions, CodeActionOrCommand,
    CodeActionParams, CodeActionProviderCapability, CompletionOptions, CompletionParams,
    CompletionResponse, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentHighlight, DocumentHighlightParams, DocumentLinkOptions,
    DocumentLinkParams, DocumentSymbolParams, DocumentSymbolResponse, ExecuteCommandOptions,
    ExecuteCommandParams, FileChangeType, FileSystemWatcher, FoldingRange, FoldingRangeParams,
    FoldingRangeProviderCapability, GlobPattern, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverParams, InitializeParams, InitializeResult, InitializedParams, InlayHint,
    InlayHintParams, Location, OneOf, PrepareRenameResponse, ReferenceParams, Registration,
    RenameOptions, RenameParams, SelectionRange, SelectionRangeParams,
    SelectionRangeProviderCapability, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    SignatureHelp, SignatureHelpOptions, SignatureHelpParams, SymbolInformation,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
    WorkDoneProgressOptions, WorkspaceEdit, WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

pub(crate) enum Task {
    Parse {
        uri: Uri,
        version: i32,
        text: String,
        incremental: Option<IncrementalEdit>,
    },
    Close {
        uri: Uri,
    },
    ExternalChange {
        uri: Uri,
        path: PathBuf,
    },
    ExternalDelete {
        uri: Uri,
    },
    CheckWorkspace {
        root: PathBuf,
    },
}

pub(crate) struct IncrementalEdit {
    pub(crate) old_green: GreenNode,
    pub(crate) old_errors: Vec<(String, TextRange)>,
    pub(crate) edit: sass_parser::reparse::TextEdit,
}

/// LSP backend state.
///
/// # Eventual consistency model
///
/// Two maps hold per-file state at different stages of the pipeline:
///
/// - **`source_texts`** — tracks which URIs are open in the editor. Updated
///   synchronously in `did_open`/`did_change`/`did_close`. Used by
///   `did_change_watched_files` to skip external changes for open files.
///
/// - **`documents`** — updated *asynchronously* by the debounced worker after parsing.
///   May lag behind `source_texts` by up to `debounce_ms`.
///
/// Read-only handlers (hover, completions, goto-def) read from `documents` and thus
/// operate on a slightly stale but internally consistent snapshot.
struct Backend {
    client: Client,
    /// Parsed state per file, updated asynchronously by the worker after debounce.
    documents: Arc<DashMap<Uri, DocumentState>>,
    /// Tracks open files. Updated synchronously in `did_open`/`did_change`/`did_close`.
    /// Used by `did_change_watched_files` to skip external changes for open files.
    source_texts: Arc<DashMap<Uri, String>>,
    module_graph: Arc<workspace::ModuleGraph>,
    runtime_config: Arc<config::RuntimeConfig>,
    task_tx: mpsc::UnboundedSender<Task>,
    /// Workspace root, captured from `initialize` for use in `didChangeConfiguration`.
    workspace_root: RwLock<Option<PathBuf>>,
}

pub(crate) struct DocumentState {
    pub(crate) version: i32,
    pub(crate) text: String,
    pub(crate) green: GreenNode,
    pub(crate) errors: Vec<(String, TextRange)>,
    pub(crate) line_index: sass_parser::line_index::LineIndex,
    pub(crate) symbols: Arc<symbols::FileSymbols>,
}

use navigation::to_lsp_document_symbol;
use semantic_tokens::{collect_semantic_tokens, delta_encode};
use worker::run_worker;

/// Compute an incremental edit by diffing old and new full texts.
///
/// Finds the single changed region by scanning from both ends and constructs
/// an `IncrementalEdit` suitable for `incremental_reparse`.
#[allow(clippy::cast_possible_truncation, dead_code)]
pub(crate) fn compute_diff_edit(
    old_green: &GreenNode,
    old_errors: &[(String, TextRange)],
    old_text: &str,
    new_text: &str,
) -> Option<IncrementalEdit> {
    let old_bytes = old_text.as_bytes();
    let new_bytes = new_text.as_bytes();

    // Common prefix length.
    let prefix = old_bytes
        .iter()
        .zip(new_bytes.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Common suffix length (not overlapping with prefix).
    let suffix = old_bytes[prefix..]
        .iter()
        .rev()
        .zip(new_bytes[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let delete = old_bytes.len() - prefix - suffix;
    let insert = new_bytes.len() - prefix - suffix;

    // Identical texts — no edit needed.
    if delete == 0 && insert == 0 {
        return None;
    }

    Some(IncrementalEdit {
        old_green: old_green.clone(),
        old_errors: old_errors.to_vec(),
        edit: sass_parser::reparse::TextEdit {
            offset: TextSize::from(u32::try_from(prefix).ok()?),
            delete: TextSize::from(u32::try_from(delete).ok()?),
            insert_len: TextSize::from(u32::try_from(insert).ok()?),
        },
    })
}

// ── LanguageServer impl ─────────────────────────────────────────────

impl LanguageServer for Backend {
    #[allow(deprecated)] // root_uri is deprecated but still widely sent by editors
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract workspace root from workspace_folders or root_uri.
        let workspace_root = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .and_then(|f| f.uri.to_file_path().map(std::borrow::Cow::into_owned))
            .or_else(|| {
                params
                    .root_uri
                    .as_ref()
                    .and_then(|u| u.to_file_path().map(std::borrow::Cow::into_owned))
            });

        let lsp_config: config::SassAnalyzerConfig = params
            .initialization_options
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        self.runtime_config.apply(&lsp_config);

        let resolver = config::build_resolver(&lsp_config, workspace_root.as_deref());
        self.module_graph.set_resolver(resolver);
        self.module_graph
            .set_prepend_imports(lsp_config.prepend_imports);

        // Build allowed roots for path traversal protection.
        // Include all workspace folders so multi-root workspaces can resolve
        // cross-folder imports.
        if let Some(root) = &workspace_root {
            let mut roots = vec![root.clone()];
            // Add additional workspace folders beyond the primary one.
            if let Some(folders) = &params.workspace_folders {
                for folder in folders.iter().skip(1) {
                    if let Some(path) = folder.uri.to_file_path() {
                        roots.push(path.into_owned());
                    }
                }
            }
            for lp in &lsp_config.load_paths {
                roots.push(root.join(lp));
            }
            for target in lsp_config.import_aliases.values() {
                for p in target.paths() {
                    roots.push(root.join(p));
                }
            }
            self.module_graph.set_allowed_roots(roots);
        }

        // Store workspace root for didChangeConfiguration.
        {
            let mut guard = self
                .workspace_root
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (*guard).clone_from(&workspace_root);
        }

        tracing::info!(?workspace_root, "configured resolver");

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::VARIABLE,
                                    SemanticTokenType::FUNCTION,
                                    SemanticTokenType::new("mixin"),
                                    SemanticTokenType::PARAMETER,
                                    SemanticTokenType::PROPERTY,
                                    SemanticTokenType::TYPE,
                                ],
                                token_modifiers: vec![
                                    SemanticTokenModifier::DECLARATION,
                                    SemanticTokenModifier::DEFINITION,
                                ],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: Some(false),
                            ..SemanticTokensOptions::default()
                        },
                    ),
                ),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "$".into(),
                        ".".into(),
                        "@".into(),
                        "\"".into(),
                        "'".into(),
                        "/".into(),
                        ":".into(),
                    ]),
                    ..CompletionOptions::default()
                }),
                hover_provider: Some(tower_lsp_server::ls_types::HoverProviderCapability::Simple(
                    true,
                )),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".into(), ",".into()]),
                    retrigger_characters: Some(vec![")".into()]),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR_EXTRACT,
                        ]),
                        ..CodeActionOptions::default()
                    },
                )),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["sass-analyzer.checkWorkspace".to_owned()],
                    ..ExecuteCommandOptions::default()
                }),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "sass-analyzer".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            offset_encoding: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let files = self.module_graph.file_count();
        let cached = self.module_graph.cached_tree_count();
        tracing::info!(files, cached, "sass-analyzer server initialized");

        // Register file watchers for SCSS/Sass files changed outside the editor.
        let watch_options = tower_lsp_server::ls_types::DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.scss".to_owned()),
                    kind: None, // defaults to Create | Change | Delete
                },
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.sass".to_owned()),
                    kind: None,
                },
            ],
        };
        let registration = Registration {
            id: "file-watcher".to_owned(),
            method: "workspace/didChangeWatchedFiles".to_owned(),
            register_options: serde_json::to_value(watch_options).ok(),
        };
        if let Err(e) = self.client.register_capability(vec![registration]).await {
            tracing::warn!(?e, "failed to register file watchers");
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        if doc.text.len() > self.runtime_config.max_file_size() {
            tracing::warn!(
                uri = ?doc.uri,
                size = doc.text.len(),
                limit = self.runtime_config.max_file_size(),
                "file exceeds size limit, skipping"
            );
            return;
        }
        self.source_texts.insert(doc.uri.clone(), doc.text.clone());

        // Eagerly parse and insert into `documents` so that goto-definition,
        // hover, etc. work immediately — before the debounced worker fires.
        // Deliberately skips `module_graph.index_file`: cross-file resolution
        // is deferred to the worker, which overwrites this entry with full
        // indexing + diagnostics after debounce.
        if let Some((green, errors)) =
            worker::parse_document(&doc.text, worker::is_sass_file(&doc.uri))
        {
            let line_index = sass_parser::line_index::LineIndex::new(&doc.text);
            let file_symbols = {
                let root = SyntaxNode::new_root(green.clone());
                Arc::new(symbols::collect_symbols(&root))
            };
            self.documents.insert(
                doc.uri.clone(),
                DocumentState {
                    version: doc.version,
                    text: doc.text.clone(),
                    green,
                    errors,
                    line_index,
                    symbols: file_symbols,
                },
            );
        }

        if self
            .task_tx
            .send(Task::Parse {
                uri: doc.uri,
                version: doc.version,
                text: doc.text,
                incremental: None,
            })
            .is_err()
        {
            tracing::error!("worker channel closed, parse task dropped");
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // FULL sync: contentChanges always contains exactly one entry with the
        // complete document text. No incremental application needed — desync
        // is impossible.
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        let text = change.text;

        if text.len() > self.runtime_config.max_file_size() {
            tracing::warn!(
                ?uri,
                size = text.len(),
                limit = self.runtime_config.max_file_size(),
                "file exceeds size limit, skipping"
            );
            return;
        }

        self.source_texts.insert(uri.clone(), text.clone());
        if self
            .task_tx
            .send(Task::Parse {
                uri,
                version,
                text,
                incremental: None,
            })
            .is_err()
        {
            tracing::error!("worker channel closed, parse task dropped");
        }
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        // VS Code wraps settings under the configurationSection key.
        let value = params
            .settings
            .get("sass-analyzer")
            .cloned()
            .unwrap_or(params.settings);
        let Ok(new_config) = serde_json::from_value::<config::SassAnalyzerConfig>(value) else {
            tracing::warn!("failed to deserialize configuration, ignoring");
            return;
        };
        self.runtime_config.apply(&new_config);

        // Rebuild resolver, prepend imports, and allowed roots from new config.
        let workspace_root = self
            .workspace_root
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let resolver = config::build_resolver(&new_config, workspace_root.as_deref());
        self.module_graph.set_resolver(resolver);
        self.module_graph
            .set_prepend_imports(new_config.prepend_imports);
        if let Some(root) = &workspace_root {
            let mut roots = vec![root.clone()];
            for lp in &new_config.load_paths {
                roots.push(root.join(lp));
            }
            for target in new_config.import_aliases.values() {
                for p in target.paths() {
                    roots.push(root.join(p));
                }
            }
            self.module_graph.set_allowed_roots(roots);
        }

        tracing::info!("configuration updated");
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {}

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for event in params.changes {
            // Skip files that are open in the editor — those are tracked
            // by did_open/did_change and have fresher content.
            if self.source_texts.contains_key(&event.uri) {
                continue;
            }

            if event.typ == FileChangeType::DELETED {
                if self
                    .task_tx
                    .send(Task::ExternalDelete { uri: event.uri })
                    .is_err()
                {
                    tracing::error!("worker channel closed, external delete task dropped");
                }
            } else {
                // Created or Changed — send path to worker for async I/O.
                let path = match event.uri.to_file_path() {
                    Some(p) => p.into_owned(),
                    None => continue,
                };
                if self
                    .task_tx
                    .send(Task::ExternalChange {
                        uri: event.uri,
                        path,
                    })
                    .is_err()
                {
                    tracing::error!("worker channel closed, external change task dropped");
                }
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.source_texts.remove(&params.text_document.uri);
        if self
            .task_tx
            .send(Task::Close {
                uri: params.text_document.uri,
            })
            .is_err()
        {
            tracing::error!("worker channel closed, close task dropped");
        }
    }

    // ── Thin dispatchers ────────────────────────────────────────────

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(doc) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let root = SyntaxNode::new_root(doc.green.clone());
        let raw = collect_semantic_tokens(&root);
        let encoded = delta_encode(&raw, &doc.text, &doc.line_index);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: encoded,
        })))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(doc) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let lsp_symbols = doc
            .symbols
            .definitions
            .iter()
            .map(|sym| to_lsp_document_symbol(sym, &doc.line_index, &doc.text))
            .collect();
        Ok(Some(DocumentSymbolResponse::Nested(lsp_symbols)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(navigation::handle_goto_definition(
            &self.documents,
            &self.module_graph,
            params,
        ))
    }

    async fn document_link(
        &self,
        params: DocumentLinkParams,
    ) -> Result<Option<Vec<tower_lsp_server::ls_types::DocumentLink>>> {
        Ok(navigation::handle_document_link(
            &self.documents,
            &self.module_graph,
            params,
        ))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        completion::handle(&self.documents, &self.module_graph, params).await
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        Ok(hover::handle(&self.documents, &self.module_graph, params))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        Ok(navigation::handle_references(
            &self.documents,
            &self.module_graph,
            params,
        ))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        Ok(navigation::handle_prepare_rename(
            &self.documents,
            &self.module_graph,
            params,
        ))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        navigation::handle_rename(&self.documents, &self.module_graph, params)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        Ok(signature_help::handle(
            &self.documents,
            &self.module_graph,
            params,
        ))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        Ok(Some(folding::handle_folding_range(&self.documents, params)))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        Ok(highlights::handle_document_highlight(
            &self.documents,
            params,
        ))
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        Ok(selection::handle_selection_range(&self.documents, params))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        Ok(inlay_hints::handle(
            &self.documents,
            &self.module_graph,
            params,
        ))
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        Ok(call_hierarchy::handle_prepare(
            &self.documents,
            &self.module_graph,
            params,
        ))
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        Ok(call_hierarchy::handle_incoming(&self.module_graph, &params))
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        Ok(call_hierarchy::handle_outgoing(&self.module_graph, &params))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<Vec<CodeActionOrCommand>>> {
        Ok(
            code_actions::handle_code_action(&self.documents, &self.module_graph, params).map(
                |actions| {
                    actions
                        .into_iter()
                        .map(CodeActionOrCommand::CodeAction)
                        .collect()
                },
            ),
        )
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        if params.command == "sass-analyzer.checkWorkspace" {
            let root = self
                .workspace_root
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(root) = root {
                let _ = self.task_tx.send(Task::CheckWorkspace { root });
            }
            return Ok(None);
        }
        Err(tower_lsp_server::jsonrpc::Error::method_not_found())
    }

    #[allow(deprecated)]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<WorkspaceSymbolResponse>> {
        let query = params.query.to_lowercase();
        let all = self.module_graph.all_symbols();

        let mut scored: Vec<(u32, SymbolInformation)> = all
            .into_iter()
            .filter_map(|(uri, sym)| {
                let score = completion::fuzzy_score(&sym.name, &query)?;
                let li = self.module_graph.line_index(&uri)?;
                let src = self.module_graph.source_text(&uri)?;
                let range = convert::text_range_to_lsp(sym.selection_range, &li, &src);
                let kind = match sym.kind {
                    symbols::SymbolKind::Variable => {
                        tower_lsp_server::ls_types::SymbolKind::VARIABLE
                    }
                    symbols::SymbolKind::Function => {
                        tower_lsp_server::ls_types::SymbolKind::FUNCTION
                    }
                    symbols::SymbolKind::Mixin => tower_lsp_server::ls_types::SymbolKind::FUNCTION,
                    symbols::SymbolKind::Placeholder => {
                        tower_lsp_server::ls_types::SymbolKind::CLASS
                    }
                };
                let container_name = uri
                    .to_file_path()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
                Some((
                    score,
                    SymbolInformation {
                        name: sym.name,
                        kind,
                        tags: None,
                        deprecated: None,
                        location: Location { uri, range },
                        container_name,
                    },
                ))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        scored.truncate(128);
        let matches: Vec<SymbolInformation> = scored.into_iter().map(|(_, si)| si).collect();

        if matches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkspaceSymbolResponse::Flat(matches)))
        }
    }
}

// ── Main ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| {
        let documents = Arc::new(DashMap::new());
        let runtime_config = Arc::new(config::RuntimeConfig::default());
        let module_graph = Arc::new(workspace::ModuleGraph::new(Arc::clone(&runtime_config)));
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_worker(
            task_rx,
            client.clone(),
            Arc::clone(&documents),
            Arc::clone(&module_graph),
            Arc::clone(&runtime_config),
        ));
        Backend {
            client,
            documents,
            source_texts: Arc::new(DashMap::new()),
            module_graph,
            runtime_config,
            task_tx,
            workspace_root: RwLock::new(None),
        }
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
