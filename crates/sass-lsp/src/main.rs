mod ast_helpers;
mod builtins;
mod completion;
mod config;
mod convert;
mod css_properties;
mod navigation;
mod semantic_tokens;
mod signature_help;
mod symbols;
mod worker;
mod workspace;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::{TextRange, TextSize};
use tokio::sync::mpsc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentLink,
    DocumentLinkOptions, DocumentLinkParams, DocumentSymbolParams, DocumentSymbolResponse,
    FileChangeType, FileSystemWatcher, GlobPattern, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverParams, InitializeParams, InitializeResult, InitializedParams, Location, OneOf,
    PrepareRenameResponse, ReferenceParams, Registration, RenameOptions, RenameParams,
    SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SignatureHelp,
    SignatureHelpOptions, SignatureHelpParams, SymbolInformation, TextDocumentContentChangeEvent,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri,
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
        text: String,
    },
    ExternalDelete {
        uri: Uri,
    },
}

pub(crate) struct IncrementalEdit {
    pub(crate) old_green: rowan::GreenNode,
    pub(crate) old_errors: Vec<(String, TextRange)>,
    pub(crate) edit: sass_parser::reparse::TextEdit,
}

/// LSP backend state.
///
/// # Eventual consistency model
///
/// Two maps hold per-file state at different stages of the pipeline:
///
/// - **`source_texts`** — updated *synchronously* in `did_open`/`did_change` on the
///   main LSP task. Always reflects the latest editor content. Cleaned on `did_close`;
///   entries may leak if a client never sends `textDocument/didClose`.
///
/// - **`documents`** — updated *asynchronously* by the debounced worker after parsing.
///   May lag behind `source_texts` by up to `DEBOUNCE_MS`.
///
/// Read-only handlers (hover, completions, goto-def) read from `documents` and thus
/// operate on a slightly stale but internally consistent snapshot.
#[allow(dead_code)]
struct Backend {
    client: Client,
    /// Parsed state per file, updated asynchronously by the worker after debounce.
    documents: Arc<DashMap<Uri, DocumentState>>,
    /// Latest source text per file, updated synchronously in `did_open`/`did_change`.
    /// Needed for incremental sync: we apply text edits here before sending to worker.
    /// Cleaned on `did_close`; may leak if the client never sends `didClose`.
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
    pub(crate) green: rowan::GreenNode,
    #[allow(dead_code)]
    pub(crate) errors: Vec<(String, TextRange)>,
    pub(crate) line_index: sass_parser::line_index::LineIndex,
    #[allow(dead_code)]
    pub(crate) symbols: Arc<symbols::FileSymbols>,
}

use completion::{
    CompletionContext, detect_completion_context, fuzzy_score, symbol_to_completion_item,
};
use convert::{apply_content_changes, lsp_pos_to_byte, lsp_position_to_offset, text_range_to_lsp};
use navigation::{
    find_definition_at_offset, find_reference_at_offset, make_hover, to_lsp_document_symbol,
};
use semantic_tokens::{collect_semantic_tokens, delta_encode};
use signature_help::{build_signature_info, count_active_parameter, find_call_at_offset};
use worker::run_worker;

#[allow(clippy::cast_possible_truncation)]
fn compute_incremental_edit(
    documents: &DashMap<Uri, DocumentState>,
    uri: &Uri,
    old_text: &str,
    changes: &[TextDocumentContentChangeEvent],
) -> Option<IncrementalEdit> {
    if changes.len() != 1 {
        return None;
    }
    let range = changes[0].range?;
    let doc = documents.get(uri)?;
    let start = lsp_pos_to_byte(old_text, range.start)?;
    let end = lsp_pos_to_byte(old_text, range.end)?;
    if start > end || end > old_text.len() {
        return None;
    }
    let delete = u32::try_from(end - start).ok()?;
    let insert_len = u32::try_from(changes[0].text.len()).ok()?;
    Some(IncrementalEdit {
        old_green: doc.green.clone(),
        old_errors: doc.errors.clone(),
        edit: sass_parser::reparse::TextEdit {
            offset: TextSize::from(start as u32),
            delete: TextSize::from(delete),
            insert_len: TextSize::from(insert_len),
        },
    })
}

use ast_helpers::name_only_range;

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

        // Build allowed roots for path traversal protection
        if let Some(root) = &workspace_root {
            let mut roots = vec![root.clone()];
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
                    TextDocumentSyncKind::INCREMENTAL,
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

        let Some(mut text) = self.source_texts.get(&uri).map(|t| t.clone()) else {
            // No prior text — take last full-content change if available.
            if let Some(change) = params.content_changes.into_iter().last() {
                if change.text.len() > self.runtime_config.max_file_size() {
                    tracing::warn!(
                        ?uri,
                        size = change.text.len(),
                        limit = self.runtime_config.max_file_size(),
                        "file exceeds size limit, skipping"
                    );
                    return;
                }
                self.source_texts.insert(uri.clone(), change.text.clone());
                if self
                    .task_tx
                    .send(Task::Parse {
                        uri,
                        version,
                        text: change.text,
                        incremental: None,
                    })
                    .is_err()
                {
                    tracing::error!("worker channel closed, parse task dropped");
                }
            }
            return;
        };

        // Compute incremental edit info before apply_content_changes consumes changes.
        let incremental =
            compute_incremental_edit(&self.documents, &uri, &text, &params.content_changes);

        if !apply_content_changes(&mut text, params.content_changes) {
            tracing::warn!(?uri, "incremental edit failed, dropping change");
            return;
        }

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
                incremental,
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
                // Created or Changed — read from disk.
                let Some(path) = event.uri.to_file_path() else {
                    continue;
                };
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if text.len() > self.runtime_config.max_file_size() {
                    continue;
                }
                if self
                    .task_tx
                    .send(Task::ExternalChange {
                        uri: event.uri,
                        text,
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
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let (green, offset) = {
            let Some(doc) = self.documents.get(&uri) else {
                return Ok(None);
            };
            let Some(offset) = lsp_position_to_offset(&doc.text, &doc.line_index, position) else {
                return Ok(None);
            };
            (doc.green.clone(), offset)
        };

        let root = SyntaxNode::new_root(green);
        let Some(ref_info) = find_reference_at_offset(&root, offset) else {
            return Ok(None);
        };

        let resolved = if let Some(namespace) = &ref_info.namespace {
            self.module_graph
                .resolve_qualified(&uri, namespace, &ref_info.name, ref_info.kind)
        } else {
            self.module_graph
                .resolve_unqualified(&uri, &ref_info.name, ref_info.kind)
        };

        let Some((target_uri, symbol)) = resolved else {
            return Ok(None);
        };

        let Some(target_line_index) = self.module_graph.line_index(&target_uri) else {
            return Ok(None);
        };
        let Some(target_source) = self.module_graph.source_text(&target_uri) else {
            return Ok(None);
        };

        let range = text_range_to_lsp(symbol.selection_range, &target_line_index, &target_source);
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: target_uri,
            range,
        })))
    }

    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let uri = params.text_document.uri;
        let Some(doc) = self.documents.get(&uri) else {
            return Ok(None);
        };

        let root = SyntaxNode::new_root(doc.green.clone());
        let line_index = &doc.line_index;
        let mut links = Vec::new();

        for node in root.descendants() {
            let kind = node.kind();
            if kind != SyntaxKind::USE_RULE
                && kind != SyntaxKind::FORWARD_RULE
                && kind != SyntaxKind::IMPORT_RULE
            {
                continue;
            }

            let Some(string_token) = node
                .children_with_tokens()
                .filter_map(rowan::NodeOrToken::into_token)
                .find(|t| t.kind() == SyntaxKind::QUOTED_STRING)
            else {
                continue;
            };

            let text = string_token.text();
            if text.len() < 2 {
                continue;
            }
            let spec = &text[1..text.len() - 1];

            let Some(target_uri) = self.module_graph.resolve_import(&uri, spec) else {
                continue;
            };

            let range = text_range_to_lsp(string_token.text_range(), line_index, &doc.text);
            links.push(DocumentLink {
                range,
                target: Some(target_uri),
                tooltip: Some(spec.to_owned()),
                data: None,
            });
        }

        if links.is_empty() {
            Ok(None)
        } else {
            Ok(Some(links))
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let cursor_line = {
            let Some(doc) = self.documents.get(&uri) else {
                return Ok(None);
            };
            let line_idx = position.line as usize;
            match doc.text.lines().nth(line_idx) {
                Some(line) => line.to_owned(),
                None => return Ok(None),
            }
        };

        let ctx = detect_completion_context(&cursor_line, position.character);

        match ctx {
            CompletionContext::UseModulePath(partial) => {
                let graph = Arc::clone(&self.module_graph);
                let uri_clone = uri.clone();
                let items = tokio::task::spawn_blocking(move || {
                    graph.complete_use_paths(&uri_clone, &partial)
                })
                .await
                .unwrap_or_default();
                if items.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(CompletionResponse::Array(items)));
            }
            CompletionContext::PropertyName(partial) => {
                let mut scored: Vec<(u32, &str)> = css_properties::CSS_PROPERTIES
                    .iter()
                    .filter_map(|p| {
                        let score = fuzzy_score(p, &partial)?;
                        Some((score, *p))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                let items: Vec<CompletionItem> = scored
                    .into_iter()
                    .map(|(score, p)| CompletionItem {
                        label: p.to_owned(),
                        kind: Some(CompletionItemKind::PROPERTY),
                        sort_text: Some(format!("0_{:04}_{p}", 1000 - score)),
                        ..CompletionItem::default()
                    })
                    .collect();
                if items.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(CompletionResponse::Array(items)));
            }
            _ => {}
        }

        let visible = self.module_graph.visible_symbols(&uri);
        if visible.is_empty() {
            return Ok(None);
        }

        let items: Vec<CompletionItem> = visible
            .into_iter()
            .filter(|(prefix, _, sym)| match &ctx {
                CompletionContext::Variable => sym.kind == symbols::SymbolKind::Variable,
                CompletionContext::IncludeMixin => sym.kind == symbols::SymbolKind::Mixin,
                CompletionContext::Namespace(ns) => prefix.as_ref().is_some_and(|p| p == ns),
                CompletionContext::Extend => sym.kind == symbols::SymbolKind::Placeholder,
                CompletionContext::General | CompletionContext::PropertyValue => true,
                CompletionContext::PropertyName(_) | CompletionContext::UseModulePath(_) => false,
            })
            .map(|(prefix, sym_uri, sym)| {
                let is_builtin = builtins::is_builtin_uri(sym_uri.as_str());
                symbol_to_completion_item(prefix.as_deref(), &sym, is_builtin)
            })
            .collect();

        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let (green, offset, file_symbols, line_index, doc_text) = {
            let Some(doc) = self.documents.get(&uri) else {
                return Ok(None);
            };
            let Some(offset) = lsp_position_to_offset(&doc.text, &doc.line_index, position) else {
                return Ok(None);
            };
            (
                doc.green.clone(),
                offset,
                doc.symbols.clone(),
                doc.line_index.clone(),
                doc.text.clone(),
            )
        };

        let root = SyntaxNode::new_root(green);

        // 1. Try reference at cursor → resolve to definition
        if let Some(ref_info) = find_reference_at_offset(&root, offset) {
            let resolved = if let Some(namespace) = &ref_info.namespace {
                self.module_graph
                    .resolve_qualified(&uri, namespace, &ref_info.name, ref_info.kind)
            } else {
                self.module_graph
                    .resolve_unqualified(&uri, &ref_info.name, ref_info.kind)
            };

            if let Some((target_uri, symbol)) = resolved {
                let source = if target_uri == uri {
                    None
                } else {
                    Some(&target_uri)
                };
                let range = Some(text_range_to_lsp(ref_info.range, &line_index, &doc_text));
                return Ok(Some(make_hover(&symbol, source, range)));
            }
            return Ok(None);
        }

        // 2. Try definition at cursor (hovering on a declaration name)
        if let Some(symbol) = find_definition_at_offset(&file_symbols, offset) {
            let range = Some(text_range_to_lsp(
                symbol.selection_range,
                &line_index,
                &doc_text,
            ));
            return Ok(Some(make_hover(symbol, None, range)));
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let (green, offset, file_symbols) = {
            let Some(doc) = self.documents.get(&uri) else {
                return Ok(None);
            };
            let Some(offset) = lsp_position_to_offset(&doc.text, &doc.line_index, position) else {
                return Ok(None);
            };
            (doc.green.clone(), offset, doc.symbols.clone())
        };

        let root = SyntaxNode::new_root(green);

        let (target_uri, target_name, target_kind) = if let Some(ref_info) =
            find_reference_at_offset(&root, offset)
        {
            let resolved = if let Some(namespace) = &ref_info.namespace {
                self.module_graph
                    .resolve_qualified(&uri, namespace, &ref_info.name, ref_info.kind)
            } else {
                self.module_graph
                    .resolve_unqualified(&uri, &ref_info.name, ref_info.kind)
            };
            let Some((target_uri, sym)) = resolved else {
                return Ok(None);
            };
            (target_uri, sym.name, sym.kind)
        } else if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
            (uri.clone(), sym.name.clone(), sym.kind)
        } else {
            return Ok(None);
        };

        let refs = self.module_graph.find_all_references(
            &target_uri,
            &target_name,
            target_kind,
            params.context.include_declaration,
        );

        if refs.is_empty() {
            return Ok(None);
        }

        let locations: Vec<Location> = refs
            .into_iter()
            .filter_map(|(ref_uri, range)| {
                let li = self.module_graph.line_index(&ref_uri)?;
                let src = self.module_graph.source_text(&ref_uri)?;
                Some(Location {
                    uri: ref_uri,
                    range: text_range_to_lsp(range, &li, &src),
                })
            })
            .collect();

        Ok(Some(locations))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        let (green, offset, file_symbols) = {
            let Some(doc) = self.documents.get(&uri) else {
                return Ok(None);
            };
            let Some(offset) = lsp_position_to_offset(&doc.text, &doc.line_index, position) else {
                return Ok(None);
            };
            (doc.green.clone(), offset, doc.symbols.clone())
        };

        let root = SyntaxNode::new_root(green);

        // Check if cursor is on a reference or definition
        if let Some(ref_info) = find_reference_at_offset(&root, offset) {
            let resolved = if let Some(namespace) = &ref_info.namespace {
                self.module_graph
                    .resolve_qualified(&uri, namespace, &ref_info.name, ref_info.kind)
            } else {
                self.module_graph
                    .resolve_unqualified(&uri, &ref_info.name, ref_info.kind)
            };
            let Some((_, sym)) = resolved else {
                return Ok(None);
            };
            let Some(li) = self.module_graph.line_index(&uri) else {
                return Ok(None);
            };
            let Some(src) = self.module_graph.source_text(&uri) else {
                return Ok(None);
            };
            let name_range = name_only_range(ref_info.kind, ref_info.range);
            return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: text_range_to_lsp(name_range, &li, &src),
                placeholder: sym.name,
            }));
        }

        if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
            let Some(li) = self.module_graph.line_index(&uri) else {
                return Ok(None);
            };
            let Some(src) = self.module_graph.source_text(&uri) else {
                return Ok(None);
            };
            let name_range = name_only_range(sym.kind, sym.selection_range);
            return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: text_range_to_lsp(name_range, &li, &src),
                placeholder: sym.name.clone(),
            }));
        }

        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let (green, offset, file_symbols) = {
            let Some(doc) = self.documents.get(&uri) else {
                return Ok(None);
            };
            let Some(offset) = lsp_position_to_offset(&doc.text, &doc.line_index, position) else {
                return Ok(None);
            };
            (doc.green.clone(), offset, doc.symbols.clone())
        };

        let root = SyntaxNode::new_root(green);

        let (target_uri, target_name, target_kind) = if let Some(ref_info) =
            find_reference_at_offset(&root, offset)
        {
            let resolved = if let Some(namespace) = &ref_info.namespace {
                self.module_graph
                    .resolve_qualified(&uri, namespace, &ref_info.name, ref_info.kind)
            } else {
                self.module_graph
                    .resolve_unqualified(&uri, &ref_info.name, ref_info.kind)
            };
            let Some((target_uri, sym)) = resolved else {
                return Ok(None);
            };
            (target_uri, sym.name, sym.kind)
        } else if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
            (uri.clone(), sym.name.clone(), sym.kind)
        } else {
            return Ok(None);
        };

        // Conflict detection: check if new_name already exists in the target file
        if self
            .module_graph
            .check_name_conflict(&target_uri, &new_name, target_kind)
        {
            let kind_label = match target_kind {
                symbols::SymbolKind::Variable => "variable",
                symbols::SymbolKind::Function => "function",
                symbols::SymbolKind::Mixin => "mixin",
                symbols::SymbolKind::Placeholder => "placeholder",
            };
            let sigil = if target_kind == symbols::SymbolKind::Variable {
                "$"
            } else if target_kind == symbols::SymbolKind::Placeholder {
                "%"
            } else {
                ""
            };
            return Err(tower_lsp_server::jsonrpc::Error {
                code: tower_lsp_server::jsonrpc::ErrorCode::InvalidParams,
                message: format!("A {kind_label} '{sigil}{new_name}' already exists in this scope")
                    .into(),
                data: None,
            });
        }

        // Find all references + declaration
        let refs = self.module_graph.find_all_references(
            &target_uri,
            &target_name,
            target_kind,
            true, // always include declaration for rename
        );

        if refs.is_empty() {
            return Ok(None);
        }

        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        for (ref_uri, range) in refs {
            let Some(li) = self.module_graph.line_index(&ref_uri) else {
                continue;
            };
            let Some(src) = self.module_graph.source_text(&ref_uri) else {
                continue;
            };
            let edit_range = name_only_range(target_kind, range);
            changes.entry(ref_uri).or_default().push(TextEdit {
                range: text_range_to_lsp(edit_range, &li, &src),
                new_text: new_name.clone(),
            });
        }

        // Update @forward show/hide clauses that mention the old name
        let forward_refs = self.module_graph.find_forward_show_hide_references(
            &target_uri,
            &target_name,
            target_kind,
        );
        for (fwd_uri, range) in forward_refs {
            let Some(li) = self.module_graph.line_index(&fwd_uri) else {
                continue;
            };
            let Some(src) = self.module_graph.source_text(&fwd_uri) else {
                continue;
            };
            changes.entry(fwd_uri).or_default().push(TextEdit {
                range: text_range_to_lsp(range, &li, &src),
                new_text: new_name.clone(),
            });
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let (green, text, offset) = {
            let Some(doc) = self.documents.get(&uri) else {
                return Ok(None);
            };
            let Some(offset) = lsp_position_to_offset(&doc.text, &doc.line_index, position) else {
                return Ok(None);
            };
            (doc.green.clone(), doc.text.clone(), offset)
        };

        let root = SyntaxNode::new_root(green);

        let Some(call_info) = find_call_at_offset(&root, offset) else {
            return Ok(None);
        };

        let resolved = if let Some(namespace) = &call_info.namespace {
            self.module_graph
                .resolve_qualified(&uri, namespace, &call_info.name, call_info.kind)
        } else {
            self.module_graph
                .resolve_unqualified(&uri, &call_info.name, call_info.kind)
        };

        let Some((_target_uri, symbol)) = resolved else {
            return Ok(None);
        };

        let Some(params_text) = &symbol.params else {
            return Ok(None);
        };

        let active_param = count_active_parameter(&text, &call_info, offset);

        let sig_info = build_signature_info(&symbol, params_text);

        Ok(Some(SignatureHelp {
            signatures: vec![sig_info],
            active_signature: Some(0),
            active_parameter: Some(active_param),
        }))
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
                let score = fuzzy_score(&sym.name, &query)?;
                let li = self.module_graph.line_index(&uri)?;
                let src = self.module_graph.source_text(&uri)?;
                let range = text_range_to_lsp(sym.selection_range, &li, &src);
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
mod tests {
    use super::*;
    use std::time::Duration;

    use serde_json::Value;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower_lsp_server::ls_types::{ParameterLabel, Position, Range};

    use crate::convert::byte_to_lsp_pos;
    use crate::semantic_tokens::{MOD_DECLARATION, TOK_VARIABLE};
    use crate::signature_help::parse_param_labels;

    /// Send a JSON-RPC message with Content-Length framing.
    async fn send_msg(writer: &mut (impl AsyncWriteExt + Unpin), msg: &Value) {
        let body = serde_json::to_string(msg).unwrap();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        writer.write_all(header.as_bytes()).await.unwrap();
        writer.write_all(body.as_bytes()).await.unwrap();
        writer.flush().await.unwrap();
    }

    /// Read one raw JSON-RPC message from the stream.
    async fn recv_msg_raw(reader: &mut (impl AsyncReadExt + Unpin)) -> Value {
        let mut header_buf = Vec::new();
        // Read until we find \r\n\r\n
        loop {
            let mut byte = [0u8; 1];
            reader.read_exact(&mut byte).await.unwrap();
            header_buf.push(byte[0]);
            if header_buf.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let header = String::from_utf8(header_buf).unwrap();
        let len: usize = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let mut body = vec![0u8; len];
        reader.read_exact(&mut body).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// Read the next client-facing message, auto-responding to any
    /// server→client requests (e.g. `workspace/semanticTokens/refresh`).
    async fn recv_msg(
        reader: &mut (impl AsyncReadExt + Unpin),
        writer: &mut (impl AsyncWriteExt + Unpin),
    ) -> Value {
        loop {
            let msg = recv_msg_raw(reader).await;
            // Server→client request: has both "id" and "method"
            if msg.get("method").is_some() && msg.get("id").is_some() {
                // Auto-respond with null result
                send_msg(
                    writer,
                    &serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": msg["id"],
                        "result": null
                    }),
                )
                .await;
                continue;
            }
            return msg;
        }
    }

    /// Spawn the LSP server on in-memory duplex streams, return client-side handles.
    fn spawn_server() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
        let (client_read, server_write) = tokio::io::duplex(1024 * 64);
        let (server_read, client_write) = tokio::io::duplex(1024 * 64);

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
        tokio::spawn(Server::new(server_read, server_write, socket).serve(service));

        (client_read, client_write)
    }

    async fn do_initialize(
        reader: &mut (impl AsyncReadExt + Unpin),
        writer: &mut (impl AsyncWriteExt + Unpin),
    ) -> Value {
        send_msg(
            writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "capabilities": {}, "rootUri": null }
            }),
        )
        .await;
        let resp = recv_msg(reader, writer).await;

        send_msg(
            writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            }),
        )
        .await;

        resp
    }

    #[tokio::test]
    async fn initialize_returns_capabilities() {
        let (mut reader, mut writer) = spawn_server();
        let resp = do_initialize(&mut reader, &mut writer).await;

        let caps = &resp["result"]["capabilities"];
        assert_eq!(caps["textDocumentSync"], 2);

        let legend = &caps["semanticTokensProvider"]["legend"];
        let types: Vec<&str> = legend["tokenTypes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            [
                "variable",
                "function",
                "mixin",
                "parameter",
                "property",
                "type"
            ]
        );

        let info = &resp["result"]["serverInfo"];
        assert_eq!(info["name"], "sass-analyzer");
    }

    #[tokio::test]
    async fn did_open_publishes_diagnostics() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///test.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": ".btn { color: red; }"
                    }
                }
            }),
        )
        .await;

        // Worker debounce fires, publishes diagnostics
        let notif = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(notif["method"], "textDocument/publishDiagnostics");
        let diags = notif["params"]["diagnostics"].as_array().unwrap();
        assert!(diags.is_empty(), "valid SCSS should have 0 diagnostics");
    }

    #[tokio::test]
    async fn did_open_with_errors_publishes_diagnostics() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///bad.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": "{{ invalid"
                    }
                }
            }),
        )
        .await;

        let notif = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(notif["method"], "textDocument/publishDiagnostics");
        let diags = notif["params"]["diagnostics"].as_array().unwrap();
        assert!(!diags.is_empty(), "invalid SCSS should have diagnostics");
    }

    #[tokio::test]
    async fn semantic_tokens_for_scss() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n.btn { color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///tokens.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;

        // Wait for diagnostics (means parse is done)
        let _diag = recv_msg(&mut reader, &mut writer).await;

        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/semanticTokens/full",
                "params": {
                    "textDocument": { "uri": "file:///tokens.scss" }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let data = resp["result"]["data"].as_array().unwrap();
        // Each token is 5 u32s; expect: $color(decl), color(prop), $color(ref)
        assert_eq!(data.len() % 5, 0, "data length must be multiple of 5");
        let token_count = data.len() / 5;
        assert_eq!(
            token_count, 3,
            "expected 3 tokens: $color decl, color prop, $color ref"
        );

        // First token: $color declaration at line 0, col 0
        assert_eq!(data[0], 0, "delta_line");
        assert_eq!(data[1], 0, "delta_start");
        assert_eq!(data[2], 6, "length of $color");
        assert_eq!(data[3], TOK_VARIABLE, "token_type = VARIABLE");
        assert_eq!(data[4], MOD_DECLARATION, "modifier = DECLARATION");
    }

    #[tokio::test]
    async fn did_close_clears_diagnostics() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///close.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": ".a { color: red; }"
                    }
                }
            }),
        )
        .await;
        let _ = recv_msg(&mut reader, &mut writer).await; // open diagnostics

        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didClose",
                "params": {
                    "textDocument": { "uri": "file:///close.scss" }
                }
            }),
        )
        .await;

        let notif = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(notif["method"], "textDocument/publishDiagnostics");
        let diags = notif["params"]["diagnostics"].as_array().unwrap();
        assert!(diags.is_empty(), "close should clear diagnostics");
    }

    #[tokio::test(start_paused = true)]
    async fn debounce_coalesces_rapid_changes() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        // Open document
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///debounce.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": ".a {}"
                    }
                }
            }),
        )
        .await;

        // Advance past debounce to get initial diagnostics
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;
        let _ = recv_msg(&mut reader, &mut writer).await;

        // Send 5 rapid changes within 20ms (well under 50ms debounce)
        for v in 2..=6 {
            send_msg(
                &mut writer,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/didChange",
                    "params": {
                        "textDocument": { "uri": "file:///debounce.scss", "version": v },
                        "contentChanges": [{ "text": format!(".v{v} {{}}") }]
                    }
                }),
            )
            .await;
            tokio::time::advance(Duration::from_millis(4)).await;
            tokio::task::yield_now().await;
        }

        // Advance past debounce deadline
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;

        // Should get exactly one diagnostics notification (coalesced)
        let notif = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(notif["method"], "textDocument/publishDiagnostics");
        // Version should be the latest (6)
        assert_eq!(notif["params"]["version"], 6);
    }

    #[tokio::test]
    async fn document_symbols_for_scss() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$primary: blue;\n@mixin btn($size) { font-size: $size; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///symbols.scss",
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
                "id": 3,
                "method": "textDocument/documentSymbol",
                "params": {
                    "textDocument": { "uri": "file:///symbols.scss" }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = resp["result"].as_array().unwrap();
        assert_eq!(result.len(), 2, "expected 2 symbols: $primary and btn");

        assert_eq!(result[0]["name"], "primary");
        assert_eq!(result[0]["kind"], 13); // SymbolKind::VARIABLE = 13

        assert_eq!(result[1]["name"], "btn");
        assert_eq!(result[1]["kind"], 12); // SymbolKind::FUNCTION = 12
        assert!(result[1]["detail"].as_str().unwrap().contains("@mixin"));
    }

    #[tokio::test]
    async fn goto_definition_variable() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n.btn { color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///def.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Cursor on $color reference at line 1, character 15
        // Line 1: ".btn { color: $color; }"
        //                        ^ char 14 ($), char 15 (c)
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 10,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": "file:///def.scss" },
                    "position": { "line": 1, "character": 15 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        assert_eq!(result["uri"], "file:///def.scss");
        // Definition: $color at line 0, characters 0..6
        assert_eq!(result["range"]["start"]["line"], 0);
        assert_eq!(result["range"]["start"]["character"], 0);
        assert_eq!(result["range"]["end"]["line"], 0);
        assert_eq!(result["range"]["end"]["character"], 6);
    }

    #[tokio::test]
    async fn goto_definition_mixin() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@mixin btn { display: block; }\n.card { @include btn; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///mixin.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Cursor on "btn" in @include btn at line 1
        // Line 1: ".card { @include btn; }"
        //                          ^ char 17
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 11,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": "file:///mixin.scss" },
                    "position": { "line": 1, "character": 17 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        assert_eq!(result["uri"], "file:///mixin.scss");
        // @mixin btn → name "btn" starts at character 7
        assert_eq!(result["range"]["start"]["line"], 0);
        assert_eq!(result["range"]["start"]["character"], 7);
    }

    #[tokio::test]
    async fn goto_definition_function() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@function double($n) { @return $n * 2; }\n.x { width: double(5px); }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///func.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Cursor on "double" call at line 1
        // Line 1: ".x { width: double(5px); }"
        //                      ^ char 12
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 12,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": "file:///func.scss" },
                    "position": { "line": 1, "character": 13 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        assert_eq!(result["uri"], "file:///func.scss");
        // @function double → name "double" at character 10
        assert_eq!(result["range"]["start"]["line"], 0);
        assert_eq!(result["range"]["start"]["character"], 10);
    }

    #[tokio::test]
    async fn goto_definition_returns_null_on_definition() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///nodef.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Cursor on $color declaration itself (should return null)
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 13,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": "file:///nodef.scss" },
                    "position": { "line": 0, "character": 1 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(
            resp["result"].is_null(),
            "definition on a decl should be null"
        );
    }

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

    #[tokio::test]
    async fn hover_on_variable_reference() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n.btn { color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///hover.scss",
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
                "id": 30,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///hover.scss" },
                    "position": { "line": 1, "character": 15 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(
            content.contains("$color"),
            "hover should show variable name"
        );
        assert!(content.contains("red"), "hover should show value");
    }

    #[tokio::test]
    async fn hover_on_variable_definition() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$primary: blue;\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///hover_def.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Hover on the $ of $primary definition
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 31,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///hover_def.scss" },
                    "position": { "line": 0, "character": 1 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(
            content.contains("$primary"),
            "hover on def should show name"
        );
        assert!(content.contains("blue"), "hover on def should show value");
    }

    #[tokio::test]
    async fn hover_on_mixin_reference() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@mixin btn($size) { font-size: $size; }\n.card { @include btn(16px); }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///hover_mixin.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Hover on "btn" in @include btn(16px)
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 32,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///hover_mixin.scss" },
                    "position": { "line": 1, "character": 17 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(content.contains("@mixin"), "hover should show @mixin");
        assert!(content.contains("btn"), "hover should show mixin name");
    }

    #[tokio::test]
    async fn hover_returns_null_on_empty_space() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n\n.btn { }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///hover_null.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Hover on empty line
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 33,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///hover_null.scss" },
                    "position": { "line": 1, "character": 0 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(
            resp["result"].is_null(),
            "hover on empty space should be null"
        );
    }

    #[tokio::test]
    async fn hover_with_doc_comment() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "/// The primary color\n$primary: #333;\n.a { color: $primary; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///hover_doc.scss",
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
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///hover_doc.scss" },
                    "position": { "line": 2, "character": 14 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(
            content.contains("primary color"),
            "hover should show doc comment"
        );
    }

    #[tokio::test]
    async fn hover_builtin_function_shows_doc_url() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@use \"sass:math\";\n.a { width: math.ceil(1.5); }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///hover_builtin.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Hover on "ceil" (line 1, character 17 = inside "ceil")
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 35,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///hover_builtin.scss" },
                    "position": { "line": 1, "character": 17 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(
            content.contains("@function ceil"),
            "hover should show function signature"
        );
        assert!(
            content.contains("sass-lang.com/documentation/modules/math/#ceil"),
            "hover should contain doc URL: {content}"
        );
    }

    #[tokio::test]
    async fn hover_builtin_variable_shows_doc_url() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@use \"sass:math\";\n.a { content: math.$pi; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///hover_builtin_var.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Hover on "pi" (line 1, character 20 = inside "$pi")
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 36,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///hover_builtin_var.scss" },
                    "position": { "line": 1, "character": 20 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(content.contains("$pi"), "hover should show variable name");
        assert!(
            content.contains("sass-lang.com/documentation/modules/math/#%24pi"),
            "hover should contain doc URL with $ anchor: {content}"
        );
    }

    #[tokio::test]
    async fn initialize_reports_hover_capability() {
        let (mut reader, mut writer) = spawn_server();
        let resp = do_initialize(&mut reader, &mut writer).await;
        let caps = &resp["result"]["capabilities"];
        assert_eq!(caps["hoverProvider"], true);
    }

    #[tokio::test]
    async fn initialize_reports_references_and_rename_capabilities() {
        let (mut reader, mut writer) = spawn_server();
        let resp = do_initialize(&mut reader, &mut writer).await;
        let caps = &resp["result"]["capabilities"];
        assert_eq!(caps["referencesProvider"], true);
        assert!(caps["renameProvider"].is_object());
    }

    #[tokio::test]
    async fn references_variable_same_file() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n.a { color: $color; }\n.b { border-color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///refs.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // References on $color usage at line 1
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 40,
                "method": "textDocument/references",
                "params": {
                    "textDocument": { "uri": "file:///refs.scss" },
                    "position": { "line": 1, "character": 15 },
                    "context": { "includeDeclaration": false }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let locs = resp["result"].as_array().unwrap();
        // Should find 2 references (not including declaration)
        assert_eq!(locs.len(), 2, "expected 2 references");
    }

    #[tokio::test]
    async fn references_include_declaration() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$x: 1;\n.a { width: $x; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///refs_decl.scss",
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
                "method": "textDocument/references",
                "params": {
                    "textDocument": { "uri": "file:///refs_decl.scss" },
                    "position": { "line": 1, "character": 13 },
                    "context": { "includeDeclaration": true }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let locs = resp["result"].as_array().unwrap();
        assert_eq!(locs.len(), 2, "expected 2: declaration + 1 ref");
    }

    #[tokio::test]
    async fn references_mixin() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@mixin btn { }\n.a { @include btn; }\n.b { @include btn; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///refs_mixin.scss",
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
                "method": "textDocument/references",
                "params": {
                    "textDocument": { "uri": "file:///refs_mixin.scss" },
                    "position": { "line": 1, "character": 17 },
                    "context": { "includeDeclaration": false }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let locs = resp["result"].as_array().unwrap();
        assert_eq!(locs.len(), 2, "expected 2 mixin references");
    }

    #[tokio::test]
    async fn references_returns_null_on_empty() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = ".a { color: red; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///refs_null.scss",
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
                "method": "textDocument/references",
                "params": {
                    "textDocument": { "uri": "file:///refs_null.scss" },
                    "position": { "line": 0, "character": 5 },
                    "context": { "includeDeclaration": true }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(resp["result"].is_null());
    }

    #[tokio::test]
    async fn prepare_rename_variable() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n.a { color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///prep_rename.scss",
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
                "id": 44,
                "method": "textDocument/prepareRename",
                "params": {
                    "textDocument": { "uri": "file:///prep_rename.scss" },
                    "position": { "line": 1, "character": 15 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        assert_eq!(result["placeholder"], "color");
    }

    #[tokio::test]
    async fn prepare_rename_on_definition() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///prep_rename_def.scss",
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
                "id": 45,
                "method": "textDocument/prepareRename",
                "params": {
                    "textDocument": { "uri": "file:///prep_rename_def.scss" },
                    "position": { "line": 0, "character": 1 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        assert_eq!(result["placeholder"], "color");
    }

    #[tokio::test]
    async fn rename_variable_single_file() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n.a { color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///rename.scss",
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
                "id": 46,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": "file:///rename.scss" },
                    "position": { "line": 1, "character": 15 },
                    "newName": "primary"
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let changes = &resp["result"]["changes"]["file:///rename.scss"];
        let edits = changes.as_array().unwrap();
        assert!(edits.len() >= 2, "expected at least 2 edits (decl + ref)");
        for edit in edits {
            assert_eq!(edit["newText"], "primary");
        }
    }

    #[tokio::test]
    async fn rename_mixin() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@mixin btn { }\n.a { @include btn; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///rename_mixin.scss",
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
                "id": 47,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": "file:///rename_mixin.scss" },
                    "position": { "line": 1, "character": 17 },
                    "newName": "button"
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let changes = &resp["result"]["changes"]["file:///rename_mixin.scss"];
        let edits = changes.as_array().unwrap();
        assert!(edits.len() >= 2, "expected at least 2 edits");
        for edit in edits {
            assert_eq!(edit["newText"], "button");
        }
    }

    #[tokio::test]
    async fn rename_returns_null_on_empty_space() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = ".a { color: red; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///rename_null.scss",
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
                "id": 48,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": "file:///rename_null.scss" },
                    "position": { "line": 0, "character": 5 },
                    "newName": "x"
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(resp["result"].is_null());
    }

    #[tokio::test]
    async fn rename_conflict_returns_error() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;\n$primary: blue;\n.a { color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///rename_conflict.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Try to rename $color to $primary — should fail (conflict)
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 80,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": "file:///rename_conflict.scss" },
                    "position": { "line": 2, "character": 15 },
                    "newName": "primary"
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(resp["error"].is_object(), "expected error response");
        let msg = resp["error"]["message"].as_str().unwrap();
        assert!(
            msg.contains("already exists"),
            "error message should mention conflict: {msg}"
        );
    }

    #[tokio::test]
    async fn rename_no_conflict_different_kind() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        // $color and @function color — different kinds, rename should succeed
        let scss = "$color: red;\n@function color() { @return red; }\n.a { color: $color; }\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///rename_no_conflict.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Rename $color to "shade" — no conflict because "shade" doesn't exist
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 81,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": "file:///rename_no_conflict.scss" },
                    "position": { "line": 2, "character": 15 },
                    "newName": "shade"
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(
            resp["error"].is_null(),
            "should succeed: no conflict with different kind"
        );
        let changes = &resp["result"]["changes"]["file:///rename_no_conflict.scss"];
        let edits = changes.as_array().unwrap();
        assert!(edits.len() >= 2, "expected at least 2 edits (decl + ref)");
    }

    // ── Signature help tests ────────────────────────────────────────

    #[tokio::test]
    async fn initialize_reports_signature_help_capability() {
        let (mut reader, mut writer) = spawn_server();
        let resp = do_initialize(&mut reader, &mut writer).await;

        let sig_help = &resp["result"]["capabilities"]["signatureHelpProvider"];
        assert!(!sig_help.is_null(), "should have signatureHelpProvider");
        let triggers: Vec<&str> = sig_help["triggerCharacters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(triggers, ["(", ","]);
    }

    #[tokio::test]
    async fn signature_help_function_call() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@function add($a, $b) { @return $a + $b; }\n.x { width: add(1px, ); }";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///sig_func.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Cursor after "add(1px, " → active param = 1
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 60,
                "method": "textDocument/signatureHelp",
                "params": {
                    "textDocument": { "uri": "file:///sig_func.scss" },
                    "position": { "line": 1, "character": 21 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        assert!(!result.is_null(), "should return signature help");
        let sigs = result["signatures"].as_array().unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0]["label"], "@function add($a, $b)");
        assert_eq!(result["activeParameter"], 1);
        let params = sigs[0]["parameters"].as_array().unwrap();
        assert_eq!(params.len(), 2);
    }

    #[tokio::test]
    async fn signature_help_mixin_include() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss =
            "@mixin btn($size, $color: red) { font-size: $size; }\n.a { @include btn(16px); }";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///sig_mixin.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Cursor after "@include btn(" → active param = 0
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 61,
                "method": "textDocument/signatureHelp",
                "params": {
                    "textDocument": { "uri": "file:///sig_mixin.scss" },
                    "position": { "line": 1, "character": 18 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        assert!(!result.is_null(), "should return signature help for mixin");
        let sigs = result["signatures"].as_array().unwrap();
        assert_eq!(sigs[0]["label"], "@mixin btn($size, $color: red)");
        assert_eq!(result["activeParameter"], 0);
        let params = sigs[0]["parameters"].as_array().unwrap();
        assert_eq!(params.len(), 2);
    }

    #[tokio::test]
    async fn signature_help_active_parameter_tracking() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@function f($a, $b, $c) { @return 0; }\n.x { width: f(1, 2, ); }";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///sig_active.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Cursor after "f(1, 2, " → active param = 2
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 62,
                "method": "textDocument/signatureHelp",
                "params": {
                    "textDocument": { "uri": "file:///sig_active.scss" },
                    "position": { "line": 1, "character": 20 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(resp["result"]["activeParameter"], 2);
    }

    #[tokio::test]
    async fn signature_help_returns_null_outside_call() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$color: red;";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///sig_null.scss",
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
                "id": 63,
                "method": "textDocument/signatureHelp",
                "params": {
                    "textDocument": { "uri": "file:///sig_null.scss" },
                    "position": { "line": 0, "character": 5 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(
            resp["result"].is_null(),
            "should be null outside function call"
        );
    }

    #[tokio::test]
    async fn signature_help_with_doc_comment() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "/// Scales a value\n@function scale($value, $factor: 2) { @return $value * $factor; }\n.x { width: scale(10px); }";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///sig_doc.scss",
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
                "id": 64,
                "method": "textDocument/signatureHelp",
                "params": {
                    "textDocument": { "uri": "file:///sig_doc.scss" },
                    "position": { "line": 2, "character": 18 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = &resp["result"];
        let sig = &result["signatures"][0];
        assert_eq!(sig["label"], "@function scale($value, $factor: 2)");
        let doc = &sig["documentation"];
        assert_eq!(doc["value"], "Scales a value");
    }

    #[test]
    fn parse_param_labels_offsets() {
        let params = "($a, $b: 1px)";
        let sig = format!("@function f{params}");
        let result = parse_param_labels(&sig, params);
        assert_eq!(result.len(), 2);
        // "$a" starts at offset 12 (after "@function f("), ends at 14
        if let ParameterLabel::LabelOffsets([s, e]) = result[0].label {
            assert_eq!(&sig[s as usize..e as usize], "$a");
        } else {
            panic!("expected LabelOffsets");
        }
        // "$b: 1px" starts at 16 (after "$a, "), ends at 23
        if let ParameterLabel::LabelOffsets([s, e]) = result[1].label {
            assert_eq!(&sig[s as usize..e as usize], "$b: 1px");
        } else {
            panic!("expected LabelOffsets");
        }
    }

    #[test]
    fn parse_param_labels_non_ascii_utf16() {
        // "ціна" is 8 bytes in UTF-8 but 4 UTF-16 code units
        let params = "($ціна, $b)";
        let sig = format!("@mixin m{params}");
        let result = parse_param_labels(&sig, params);
        assert_eq!(result.len(), 2);
        // "$ціна": starts at UTF-16 offset 8 ("@mixin m("), 5 UTF-16 code units long
        if let ParameterLabel::LabelOffsets([s, e]) = result[0].label {
            assert_eq!(s, 9); // "@mixin m(" = 9 UTF-16 units
            assert_eq!(e, 14); // "$ціна" = 5 UTF-16 units ($+ц+і+н+а)
        } else {
            panic!("expected LabelOffsets");
        }
        // "$b": after "$ціна, "
        if let ParameterLabel::LabelOffsets([s, e]) = result[1].label {
            assert_eq!(e - s, 2); // "$b" = 2 UTF-16 units
        } else {
            panic!("expected LabelOffsets");
        }
    }

    // ── Workspace symbol tests ──────────────────────────────────────

    #[tokio::test]
    async fn initialize_reports_workspace_symbol_capability() {
        let (mut reader, mut writer) = spawn_server();
        let resp = do_initialize(&mut reader, &mut writer).await;
        let caps = &resp["result"]["capabilities"];
        assert_eq!(caps["workspaceSymbolProvider"], true);
    }

    #[tokio::test]
    async fn workspace_symbol_returns_matching_symbols() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$primary: blue;\n@mixin btn($size) { }\n@function scale($n) { @return $n; }";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///ws.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Search for "btn"
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 70,
                "method": "workspace/symbol",
                "params": { "query": "btn" }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = resp["result"].as_array().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "btn");
    }

    #[tokio::test]
    async fn workspace_symbol_empty_query_returns_all() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$a: 1;\n$b: 2;";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///ws_all.scss",
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
                "id": 71,
                "method": "workspace/symbol",
                "params": { "query": "" }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = resp["result"].as_array().unwrap();
        assert_eq!(result.len(), 2, "empty query should return all symbols");
    }

    #[tokio::test]
    async fn workspace_symbol_fuzzy_matching() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "@mixin responsive-grid { }\n@mixin simple { }";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///ws_fuzz.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // "rg" should fuzzy-match "responsive-grid" but not "simple"
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 72,
                "method": "workspace/symbol",
                "params": { "query": "rg" }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = resp["result"].as_array().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "responsive-grid");
    }

    #[tokio::test]
    async fn workspace_symbol_no_match_returns_null() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$x: 1;";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///ws_none.scss",
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
                "id": 73,
                "method": "workspace/symbol",
                "params": { "query": "zzz" }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        assert!(resp["result"].is_null(), "no match should return null");
    }

    #[test]
    fn fuzzy_score_basics() {
        // Exact match → highest score.
        assert_eq!(super::fuzzy_score("color", "color"), Some(1000));
        // Prefix match → 500+.
        assert!(super::fuzzy_score("color-primary", "color").unwrap() >= 500);
        // Word boundary match → 200+ (r and g match starts of "responsive" and "grid").
        let rg_score = super::fuzzy_score("responsive-grid", "rg").unwrap();
        assert!(
            rg_score >= 200,
            "word boundary should score 200+, got {rg_score}"
        );
        // Subsequence match → >0.
        assert!(super::fuzzy_score("primary", "pry").unwrap() > 0);
        // No match → None.
        assert_eq!(super::fuzzy_score("simple", "rg"), None);
        // Empty query → matches everything.
        assert_eq!(super::fuzzy_score("anything", ""), Some(0));
    }

    #[test]
    fn fuzzy_score_ranking() {
        let exact = super::fuzzy_score("color", "color").unwrap();
        let prefix = super::fuzzy_score("color-primary", "color").unwrap();
        let boundary = super::fuzzy_score("responsive-grid", "rg").unwrap();
        let subseq = super::fuzzy_score("primary", "pry").unwrap();
        assert!(exact > prefix, "exact > prefix");
        assert!(prefix > boundary, "prefix > boundary");
        assert!(boundary > subseq, "boundary > subsequence");
    }

    #[test]
    fn fuzzy_score_camel_case_boundary() {
        // camelCase boundary: "bc" matches "B" from "border" and "C" from "Color"
        let score = super::fuzzy_score("borderColor", "bc").unwrap();
        assert!(score >= 200, "camelCase boundary match, got {score}");
    }

    #[test]
    fn completion_context_detection() {
        use crate::completion::{CompletionContext, detect_completion_context};

        // After `$` → Variable
        let ctx = detect_completion_context("  color: $", 10);
        assert!(matches!(ctx, CompletionContext::Variable));

        // After `@include ` → IncludeMixin
        let ctx = detect_completion_context("  @include ", 11);
        assert!(matches!(ctx, CompletionContext::IncludeMixin));

        // After `@use "` → UseModulePath
        let ctx = detect_completion_context("  @use \"", 8);
        assert!(matches!(ctx, CompletionContext::UseModulePath(_)));

        // On `bor` → PropertyName
        let ctx = detect_completion_context("  bor", 5);
        assert!(matches!(ctx, CompletionContext::PropertyName(_)));

        // After `color:` → PropertyValue
        let ctx = detect_completion_context("  color: ", 8);
        assert!(matches!(ctx, CompletionContext::PropertyValue));
    }

    async fn do_initialize_with(
        reader: &mut (impl AsyncReadExt + Unpin),
        writer: &mut (impl AsyncWriteExt + Unpin),
        init_options: Value,
    ) -> Value {
        send_msg(
            writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "capabilities": {},
                    "rootUri": "file:///project",
                    "initializationOptions": init_options
                }
            }),
        )
        .await;
        let resp = recv_msg(reader, writer).await;

        send_msg(
            writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            }),
        )
        .await;

        resp
    }

    #[tokio::test]
    async fn initialize_with_config() {
        let (mut reader, mut writer) = spawn_server();
        let resp = do_initialize_with(
            &mut reader,
            &mut writer,
            serde_json::json!({
                "loadPaths": ["src/sass"],
                "importAliases": { "@sass": "src/sass" },
                "prependImports": ["variables"]
            }),
        )
        .await;
        assert!(resp["result"]["capabilities"].is_object());
    }

    #[tokio::test]
    async fn initialize_with_empty_config() {
        let (mut reader, mut writer) = spawn_server();
        let resp = do_initialize_with(&mut reader, &mut writer, serde_json::json!({})).await;
        assert!(resp["result"]["capabilities"].is_object());
    }

    #[test]
    fn lsp_pos_to_byte_ascii() {
        let text = "abc\ndef\nghi";
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 0)), Some(0));
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 2)), Some(2));
        assert_eq!(lsp_pos_to_byte(text, Position::new(1, 0)), Some(4));
        assert_eq!(lsp_pos_to_byte(text, Position::new(1, 2)), Some(6));
        assert_eq!(lsp_pos_to_byte(text, Position::new(2, 1)), Some(9));
    }

    #[test]
    fn lsp_pos_to_byte_multibyte() {
        // "ä" is 2 UTF-8 bytes, 1 UTF-16 code unit
        let text = "äbc";
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 0)), Some(0));
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 1)), Some(2)); // after ä
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 2)), Some(3)); // after b
    }

    #[test]
    fn lsp_pos_to_byte_emoji() {
        // "😀" is 4 UTF-8 bytes, 2 UTF-16 code units (surrogate pair)
        let text = "😀b";
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 0)), Some(0));
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 2)), Some(4)); // after 😀
        assert_eq!(lsp_pos_to_byte(text, Position::new(0, 3)), Some(5)); // after b
    }

    #[test]
    fn lsp_pos_to_byte_out_of_bounds() {
        let text = "ab";
        // Line 1 doesn't exist (no newline)
        assert_eq!(lsp_pos_to_byte(text, Position::new(1, 0)), None);
    }

    #[test]
    fn apply_content_changes_insert() {
        let mut text = String::from("ab\ncd");
        let changes = vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 1), Position::new(0, 1))),
            range_length: None,
            text: "X".into(),
        }];
        assert!(apply_content_changes(&mut text, changes));
        assert_eq!(text, "aXb\ncd");
    }

    #[test]
    fn apply_content_changes_delete() {
        let mut text = String::from("abcd");
        let changes = vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 1), Position::new(0, 3))),
            range_length: None,
            text: String::new(),
        }];
        assert!(apply_content_changes(&mut text, changes));
        assert_eq!(text, "ad");
    }

    #[test]
    fn apply_content_changes_replace_across_lines() {
        let mut text = String::from("ab\ncd\nef");
        let changes = vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 1), Position::new(1, 1))),
            range_length: None,
            text: "XY".into(),
        }];
        assert!(apply_content_changes(&mut text, changes));
        assert_eq!(text, "aXYd\nef");
    }

    #[test]
    fn apply_content_changes_full_replacement() {
        let mut text = String::from("old");
        let changes = vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "new content".into(),
        }];
        assert!(apply_content_changes(&mut text, changes));
        assert_eq!(text, "new content");
    }

    #[test]
    fn apply_content_changes_sequential() {
        let mut text = String::from("abc");
        // Two sequential changes: insert X at pos 1, then insert Y at pos 3
        // After first: "aXbc", after second: "aXbYc"
        let changes = vec![
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(0, 1), Position::new(0, 1))),
                range_length: None,
                text: "X".into(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(0, 3), Position::new(0, 3))),
                range_length: None,
                text: "Y".into(),
            },
        ];
        assert!(apply_content_changes(&mut text, changes));
        assert_eq!(text, "aXbYc");
    }

    // ── Non-ASCII / UTF-16 tests ─────────────────────────────────────

    #[test]
    fn byte_to_lsp_pos_ascii() {
        let source = "abc\ndef";
        let li = sass_parser::line_index::LineIndex::new(source);
        // 'd' is at byte 4 (line 2, col 1)
        let (line, col) = byte_to_lsp_pos(source, &li, 4.into());
        assert_eq!((line, col), (1, 0));
    }

    #[test]
    fn byte_to_lsp_pos_multibyte() {
        // 'é' is 2 bytes in UTF-8 but 1 UTF-16 code unit
        let source = "café\nx";
        let li = sass_parser::line_index::LineIndex::new(source);
        // 'x' is at byte 6 (c=1, a=1, f=1, é=2, \n=1)
        let (line, col) = byte_to_lsp_pos(source, &li, 6.into());
        assert_eq!((line, col), (1, 0));
        // end of "café" = byte 5, col should be 4 (c, a, f, é each 1 UTF-16 unit)
        let (line, col) = byte_to_lsp_pos(source, &li, 5.into());
        assert_eq!((line, col), (0, 4));
    }

    #[test]
    fn byte_to_lsp_pos_surrogate_pair() {
        // '𝕊' (U+1D54A) is 4 bytes in UTF-8 and 2 UTF-16 code units (surrogate pair)
        let source = "a𝕊b\nx";
        let li = sass_parser::line_index::LineIndex::new(source);
        // 'b' is at byte 5 (a=1, 𝕊=4), should be UTF-16 col 3 (a=1, 𝕊=2)
        let (line, col) = byte_to_lsp_pos(source, &li, 5.into());
        assert_eq!((line, col), (0, 3));
    }

    #[test]
    fn apply_content_changes_multibyte() {
        // LSP positions use UTF-16 columns. 'é' is 1 UTF-16 unit.
        let mut text = String::from("café");
        // Insert 'X' after 'f' (UTF-16 col 3)
        let changes = vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 3), Position::new(0, 3))),
            range_length: None,
            text: "X".into(),
        }];
        assert!(apply_content_changes(&mut text, changes));
        assert_eq!(text, "cafXé");
    }

    #[test]
    fn apply_content_changes_surrogate_pair() {
        // '𝕊' (U+1D54A) is 2 UTF-16 code units.
        let mut text = String::from("a𝕊b");
        // Delete 'b' at UTF-16 col 3 (a=1, 𝕊=2)
        let changes = vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 3), Position::new(0, 4))),
            range_length: None,
            text: String::new(),
        }];
        assert!(apply_content_changes(&mut text, changes));
        assert_eq!(text, "a𝕊");
    }

    #[tokio::test]
    async fn diagnostics_with_non_ascii_content() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        // Open a file with Ukrainian text and an intentional parse error
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///unicode.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": "$колір: #fff;\n.кнопка { color: $колір; }"
                    }
                }
            }),
        )
        .await;

        let diag = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(diag["method"], "textDocument/publishDiagnostics");
        let diagnostics = diag["params"]["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics.is_empty(),
            "valid SCSS with non-ASCII should produce no errors, got: {diagnostics:?}"
        );
    }

    #[tokio::test]
    async fn incremental_sync_updates_diagnostics() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        // Open with valid SCSS
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///incr.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": "$x: 1;\n.a { color: red; }"
                    }
                }
            }),
        )
        .await;

        let notif = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(notif["method"], "textDocument/publishDiagnostics");
        let diags = notif["params"]["diagnostics"].as_array().unwrap();
        assert!(diags.is_empty(), "valid SCSS should have 0 diagnostics");

        // Send incremental change: replace "red" with "blue"
        // "red" is at line 1, col 14..17
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": "file:///incr.scss", "version": 2 },
                    "contentChanges": [{
                        "range": {
                            "start": { "line": 1, "character": 14 },
                            "end": { "line": 1, "character": 17 }
                        },
                        "text": "blue"
                    }]
                }
            }),
        )
        .await;

        let notif = recv_msg(&mut reader, &mut writer).await;
        assert_eq!(notif["method"], "textDocument/publishDiagnostics");
        let diags = notif["params"]["diagnostics"].as_array().unwrap();
        assert!(diags.is_empty(), "incremental edit should still be valid");
    }

    #[tokio::test]
    async fn incremental_sync_hover_after_edit() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        // Open with a variable
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///incr2.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": "$color: red;\n.a { color: $color; }"
                    }
                }
            }),
        )
        .await;
        let _ = recv_msg(&mut reader, &mut writer).await; // diagnostics

        // Incremental change: replace "red" with "blue" (line 0, col 8..11)
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": "file:///incr2.scss", "version": 2 },
                    "contentChanges": [{
                        "range": {
                            "start": { "line": 0, "character": 8 },
                            "end": { "line": 0, "character": 11 }
                        },
                        "text": "blue"
                    }]
                }
            }),
        )
        .await;
        let _ = recv_msg(&mut reader, &mut writer).await; // diagnostics

        // Hover on $color reference — should show updated value "blue"
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///incr2.scss" },
                    "position": { "line": 1, "character": 16 }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let contents = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(
            contents.contains("blue"),
            "hover should reflect incremental edit, got: {contents}"
        );
    }

    #[tokio::test]
    async fn external_change_updates_module_graph() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        // Open a file
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///ext.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": "$ext: 1;\n"
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Simulate an external change notification for a different file.
        // Since did_change_watched_files reads from disk and we can't
        // set up real files in this test, we verify that the notification
        // method is accepted without error by sending it as JSON-RPC.
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWatchedFiles",
                "params": {
                    "changes": [{
                        "uri": "file:///nonexistent.scss",
                        "type": 2
                    }]
                }
            }),
        )
        .await;

        // The server should not crash; verify it still responds.
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 100,
                "method": "textDocument/documentSymbol",
                "params": {
                    "textDocument": { "uri": "file:///ext.scss" }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = resp["result"].as_array().unwrap();
        assert_eq!(
            result.len(),
            1,
            "should still have 1 symbol after ext change"
        );
    }

    #[tokio::test]
    async fn external_delete_does_not_crash() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        // Open a file
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///del.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": "$del: 1;\n"
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // External delete of a file that's not open (should be processed).
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWatchedFiles",
                "params": {
                    "changes": [{
                        "uri": "file:///some_dep.scss",
                        "type": 3
                    }]
                }
            }),
        )
        .await;

        // Small delay for the worker to process the delete task.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Server should still be alive.
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 101,
                "method": "textDocument/documentSymbol",
                "params": {
                    "textDocument": { "uri": "file:///del.scss" }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = resp["result"].as_array().unwrap();
        assert_eq!(result.len(), 1, "should still have 1 symbol");
    }

    #[tokio::test]
    async fn watched_files_skips_open_files() {
        let (mut reader, mut writer) = spawn_server();
        do_initialize(&mut reader, &mut writer).await;

        let scss = "$open: 1;\n";
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///open.scss",
                        "languageId": "scss",
                        "version": 1,
                        "text": scss
                    }
                }
            }),
        )
        .await;
        let _diag = recv_msg(&mut reader, &mut writer).await;

        // Notify file change for an open file — should be skipped
        // because open files are tracked by did_open/did_change.
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWatchedFiles",
                "params": {
                    "changes": [{
                        "uri": "file:///open.scss",
                        "type": 2
                    }]
                }
            }),
        )
        .await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Server should still work correctly with the original content.
        send_msg(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 102,
                "method": "textDocument/documentSymbol",
                "params": {
                    "textDocument": { "uri": "file:///open.scss" }
                }
            }),
        )
        .await;

        let resp = recv_msg(&mut reader, &mut writer).await;
        let result = resp["result"].as_array().unwrap();
        assert_eq!(result.len(), 1, "should still have the original symbol");
        assert_eq!(result[0]["name"], "open");
    }
}
