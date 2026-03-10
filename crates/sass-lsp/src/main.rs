mod builtins;
mod config;
mod symbols;
mod workspace;

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use sass_parser::syntax::{SyntaxNode, SyntaxToken};
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;
use tokio::sync::mpsc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentLink, DocumentLinkOptions, DocumentLinkParams,
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverContents, HoverParams, InitializeParams, InitializeResult, InitializedParams,
    Location, MarkupContent, MarkupKind, OneOf, ParameterInformation, ParameterLabel, Position,
    PrepareRenameResponse, Range, ReferenceParams, RenameOptions, RenameParams, SemanticToken,
    SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SignatureHelp,
    SignatureHelpOptions, SignatureHelpParams, SignatureInformation, SymbolInformation,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri,
    WorkDoneProgressOptions, WorkspaceEdit, WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

const MAX_FILE_SIZE: usize = 2_000_000;
const DEBOUNCE_MS: u64 = 50;

// Semantic token type indices (must match legend order in initialize)
const TOK_VARIABLE: u32 = 0;
const TOK_FUNCTION: u32 = 1;
const TOK_MACRO: u32 = 2;
const TOK_PARAMETER: u32 = 3;
const TOK_PROPERTY: u32 = 4;
const TOK_TYPE: u32 = 5;

const MOD_DECLARATION: u32 = 1 << 0;

enum Task {
    Parse {
        uri: Uri,
        version: i32,
        text: String,
    },
    Close {
        uri: Uri,
    },
}

#[allow(dead_code)]
struct Backend {
    client: Client,
    documents: Arc<DashMap<Uri, DocumentState>>,
    module_graph: Arc<workspace::ModuleGraph>,
    task_tx: mpsc::UnboundedSender<Task>,
}

struct DocumentState {
    version: i32,
    text: String,
    green: rowan::GreenNode,
    #[allow(dead_code)]
    errors: Vec<(String, TextRange)>,
    line_index: sass_parser::line_index::LineIndex,
    #[allow(dead_code)]
    symbols: symbols::FileSymbols,
}

fn parse_document(text: &str) -> Option<(rowan::GreenNode, Vec<(String, TextRange)>)> {
    std::panic::catch_unwind(AssertUnwindSafe(|| sass_parser::parse(text))).ok()
}

fn errors_to_diagnostics(
    errors: &[(String, TextRange)],
    line_index: &sass_parser::line_index::LineIndex,
) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|(msg, range)| {
            let start = line_index.line_col(range.start());
            let end = line_index.line_col(range.end());
            Diagnostic {
                range: Range::new(
                    Position::new(start.line - 1, start.col - 1),
                    Position::new(end.line - 1, end.col - 1),
                ),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("sass-analyzer".to_owned()),
                message: msg.clone(),
                ..Diagnostic::default()
            }
        })
        .collect()
}

// ── Semantic tokens ─────────────────────────────────────────────────

struct RawSemanticToken {
    start: u32,
    len: u32,
    token_type: u32,
    modifiers: u32,
}

/// Convert byte offset → (0-based line, 0-based UTF-16 column).
#[allow(clippy::cast_possible_truncation)]
fn byte_to_lsp_pos(
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
    offset: sass_parser::text_range::TextSize,
) -> (u32, u32) {
    let lc = line_index.line_col(offset);
    let line_0 = lc.line - 1;
    let byte_offset = u32::from(offset) as usize;
    let line_start_byte = byte_offset - (lc.col as usize - 1);
    let slice = &source[line_start_byte..byte_offset];
    let col_utf16 = slice.encode_utf16().count() as u32;
    (line_0, col_utf16)
}

/// UTF-16 length of a string slice.
#[allow(clippy::cast_possible_truncation)]
fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// Find the first IDENT token among direct children.
fn first_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::IDENT)
}

/// Find the Nth IDENT token (0-indexed) among direct children.
fn nth_ident_token(node: &SyntaxNode, n: usize) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|t| t.kind() == SyntaxKind::IDENT)
        .nth(n)
}

/// Compute combined range from DOLLAR to the following IDENT in direct children.
fn dollar_ident_range(node: &SyntaxNode) -> Option<(TextRange, u32)> {
    let mut dollar_start = None;
    let mut ident_end = None;
    let mut ident_len_utf16 = 0u32;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::DOLLAR => dollar_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if dollar_start.is_some() => {
                    ident_end = Some(token.text_range().end());
                    // $name → UTF-16 length = 1 (for $) + ident chars
                    ident_len_utf16 = 1 + utf16_len(token.text());
                    break;
                }
                _ => {}
            }
        }
    }
    let start = dollar_start?;
    let end = ident_end?;
    Some((TextRange::new(start, end), ident_len_utf16))
}

fn collect_semantic_tokens(root: &SyntaxNode) -> Vec<RawSemanticToken> {
    let mut tokens = Vec::new();

    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::VARIABLE_DECL => {
                if let Some((range, len)) = dollar_ident_range(&node) {
                    tokens.push(RawSemanticToken {
                        start: range.start().into(),
                        len,
                        token_type: TOK_VARIABLE,
                        modifiers: MOD_DECLARATION,
                    });
                }
            }
            SyntaxKind::VARIABLE_REF => {
                if let Some((range, len)) = dollar_ident_range(&node) {
                    tokens.push(RawSemanticToken {
                        start: range.start().into(),
                        len,
                        token_type: TOK_VARIABLE,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::FUNCTION_CALL => {
                if let Some(ident) = first_ident_token(&node) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_FUNCTION,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::FUNCTION_RULE => {
                // Skip first IDENT ("function"), take second (the name)
                if let Some(ident) = nth_ident_token(&node, 1) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_FUNCTION,
                        modifiers: MOD_DECLARATION,
                    });
                }
            }
            SyntaxKind::MIXIN_RULE => {
                if let Some(ident) = nth_ident_token(&node, 1) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_MACRO,
                        modifiers: MOD_DECLARATION,
                    });
                }
            }
            SyntaxKind::INCLUDE_RULE => {
                if let Some(ident) = nth_ident_token(&node, 1) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_MACRO,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::PARAM => {
                if let Some((range, len)) = dollar_ident_range(&node) {
                    tokens.push(RawSemanticToken {
                        start: range.start().into(),
                        len,
                        token_type: TOK_PARAMETER,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::PROPERTY => {
                if let Some(ident) = first_ident_token(&node) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_PROPERTY,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::SIMPLE_SELECTOR => {
                // %placeholder → TYPE
                let mut has_percent = false;
                let mut pct_start = None;
                let mut ident_text = None;
                for element in node.children_with_tokens() {
                    if let Some(token) = element.into_token() {
                        match token.kind() {
                            SyntaxKind::PERCENT => {
                                has_percent = true;
                                pct_start = Some(token.text_range().start());
                            }
                            SyntaxKind::IDENT if has_percent => {
                                ident_text = Some(token.text().to_string());
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                if let (Some(start), Some(text)) = (pct_start, ident_text) {
                    tokens.push(RawSemanticToken {
                        start: start.into(),
                        len: 1 + utf16_len(&text), // % + name
                        token_type: TOK_TYPE,
                        modifiers: 0,
                    });
                }
            }
            _ => {}
        }
    }

    tokens.sort_by_key(|t| t.start);
    tokens
}

fn delta_encode(
    raw: &[RawSemanticToken],
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
) -> Vec<SemanticToken> {
    let mut result = Vec::with_capacity(raw.len());
    let mut prev_line: u32 = 0;
    let mut prev_col: u32 = 0;

    for tok in raw {
        let (line, col) = byte_to_lsp_pos(
            source,
            line_index,
            sass_parser::text_range::TextSize::from(tok.start),
        );

        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 { col - prev_col } else { col };

        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.len,
            token_type: tok.token_type,
            token_modifiers_bitset: tok.modifiers,
        });

        prev_line = line;
        prev_col = col;
    }

    result
}

// ── Worker ──────────────────────────────────────────────────────────

async fn run_worker(
    mut rx: mpsc::UnboundedReceiver<Task>,
    client: Client,
    documents: Arc<DashMap<Uri, DocumentState>>,
    module_graph: Arc<workspace::ModuleGraph>,
) {
    let mut pending: HashMap<Uri, (i32, String)> = HashMap::new();
    let debounce = Duration::from_millis(DEBOUNCE_MS);
    let sleep = tokio::time::sleep(debounce);
    tokio::pin!(sleep);
    let mut has_pending = false;

    loop {
        tokio::select! {
            task = rx.recv() => {
                let Some(task) = task else { break };
                match task {
                    Task::Parse { uri, version, text } => {
                        pending.insert(uri, (version, text));
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
                }
            }
            () = &mut sleep, if has_pending => {
                for (uri, (version, text)) in pending.drain() {
                    let Some((green, errors)) = parse_document(&text) else {
                        tracing::error!("parser panic for {uri:?}");
                        continue;
                    };
                    let line_index = sass_parser::line_index::LineIndex::new(&text);
                    let diagnostics = errors_to_diagnostics(&errors, &line_index);
                    let file_symbols = {
                        let root = SyntaxNode::new_root(green.clone());
                        symbols::collect_symbols(&root)
                    };

                    let is_current = documents
                        .get(&uri)
                        .is_none_or(|state| state.version <= version);

                    module_graph.index_file(
                        &uri,
                        green.clone(),
                        file_symbols.clone(),
                        line_index.clone(),
                    );

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
                            .publish_diagnostics(uri, diagnostics, Some(version))
                            .await;
                    }
                }
                has_pending = false;
            }
        }
    }
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

        let resolver = config::build_resolver(&lsp_config, workspace_root.as_deref());
        self.module_graph.set_resolver(resolver);
        self.module_graph
            .set_prepend_imports(lsp_config.prepend_imports);

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
                                    SemanticTokenType::MACRO,
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
                    trigger_characters: Some(vec!["$".into(), ".".into()]),
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
                    retrigger_characters: None,
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
        tracing::info!("sass-analyzer server initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        if doc.text.len() > MAX_FILE_SIZE {
            return;
        }
        let _ = self.task_tx.send(Task::Parse {
            uri: doc.uri,
            version: doc.version,
            text: doc.text,
        });
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        if change.text.len() > MAX_FILE_SIZE {
            return;
        }
        let _ = self.task_tx.send(Task::Parse {
            uri,
            version,
            text: change.text,
        });
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let _ = self.task_tx.send(Task::Close {
            uri: params.text_document.uri,
        });
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
            .map(|sym| to_lsp_document_symbol(sym, &doc.line_index))
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

        let range = text_range_to_lsp(symbol.selection_range, &target_line_index);
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

            let range = text_range_to_lsp(string_token.text_range(), line_index);
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

        let visible = self.module_graph.visible_symbols(&uri);
        if visible.is_empty() {
            return Ok(None);
        }

        let items: Vec<CompletionItem> = visible
            .into_iter()
            .map(|(prefix, _sym_uri, sym)| symbol_to_completion_item(prefix.as_deref(), &sym))
            .collect();

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

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
                return Ok(Some(make_hover(&symbol, source)));
            }
            return Ok(None);
        }

        // 2. Try definition at cursor (hovering on a declaration name)
        if let Some(symbol) = find_definition_at_offset(&file_symbols, offset) {
            return Ok(Some(make_hover(symbol, None)));
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
                Some(Location {
                    uri: ref_uri,
                    range: text_range_to_lsp(range, &li),
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
            let name_range = name_only_range(ref_info.kind, ref_info.range);
            return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: text_range_to_lsp(name_range, &li),
                placeholder: sym.name,
            }));
        }

        if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
            let Some(li) = self.module_graph.line_index(&uri) else {
                return Ok(None);
            };
            let name_range = name_only_range(sym.kind, sym.selection_range);
            return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: text_range_to_lsp(name_range, &li),
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
            let edit_range = name_only_range(target_kind, range);
            changes.entry(ref_uri).or_default().push(TextEdit {
                range: text_range_to_lsp(edit_range, &li),
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

        let mut matches: Vec<SymbolInformation> = all
            .into_iter()
            .filter(|(_, sym)| fuzzy_match(&sym.name, &query))
            .filter_map(|(uri, sym)| {
                let li = self.module_graph.line_index(&uri)?;
                let range = text_range_to_lsp(sym.selection_range, &li);
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
                Some(SymbolInformation {
                    name: sym.name,
                    kind,
                    tags: None,
                    deprecated: None,
                    location: Location { uri, range },
                    container_name: None,
                })
            })
            .collect();

        matches.sort_by(|a, b| a.name.cmp(&b.name));

        if matches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkspaceSymbolResponse::Flat(matches)))
        }
    }
}

fn symbol_to_completion_item(prefix: Option<&str>, sym: &symbols::Symbol) -> CompletionItem {
    let (label, insert_text, kind, detail) = match sym.kind {
        symbols::SymbolKind::Variable => {
            let label = if let Some(ns) = prefix {
                format!("{ns}.${}", sym.name)
            } else {
                format!("${}", sym.name)
            };
            (label, None, CompletionItemKind::VARIABLE, None)
        }
        symbols::SymbolKind::Function => {
            let label = if let Some(ns) = prefix {
                format!("{ns}.{}", sym.name)
            } else {
                sym.name.clone()
            };
            let detail = sym.params.clone();
            (label, None, CompletionItemKind::FUNCTION, detail)
        }
        symbols::SymbolKind::Mixin => {
            let label = if let Some(ns) = prefix {
                format!("{ns}.{}", sym.name)
            } else {
                sym.name.clone()
            };
            let detail = Some(
                sym.params
                    .as_ref()
                    .map_or_else(|| "@mixin".to_owned(), |p| format!("@mixin{p}")),
            );
            (label, None, CompletionItemKind::METHOD, detail)
        }
        symbols::SymbolKind::Placeholder => {
            let label = format!("%{}", sym.name);
            (label, None, CompletionItemKind::CLASS, None)
        }
    };

    let sort_text = Some(format!(
        "{}{}",
        if prefix.is_some() { "1" } else { "0" },
        &label,
    ));

    CompletionItem {
        label,
        kind: Some(kind),
        detail,
        insert_text,
        sort_text,
        ..CompletionItem::default()
    }
}

#[allow(deprecated)]
fn to_lsp_document_symbol(
    sym: &symbols::Symbol,
    line_index: &sass_parser::line_index::LineIndex,
) -> tower_lsp_server::ls_types::DocumentSymbol {
    let range = text_range_to_lsp(sym.range, line_index);
    let selection_range = text_range_to_lsp(sym.selection_range, line_index);
    let (kind, detail) = match sym.kind {
        symbols::SymbolKind::Variable => (tower_lsp_server::ls_types::SymbolKind::VARIABLE, None),
        symbols::SymbolKind::Function => (
            tower_lsp_server::ls_types::SymbolKind::FUNCTION,
            sym.params.clone(),
        ),
        symbols::SymbolKind::Mixin => (
            tower_lsp_server::ls_types::SymbolKind::FUNCTION,
            Some(
                sym.params
                    .as_ref()
                    .map_or_else(|| "@mixin".to_owned(), |p| format!("@mixin{p}")),
            ),
        ),
        symbols::SymbolKind::Placeholder => (tower_lsp_server::ls_types::SymbolKind::CLASS, None),
    };
    tower_lsp_server::ls_types::DocumentSymbol {
        name: sym.name.clone(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: None,
    }
}

fn text_range_to_lsp(
    range: sass_parser::text_range::TextRange,
    line_index: &sass_parser::line_index::LineIndex,
) -> Range {
    let start = line_index.line_col(range.start());
    let end = line_index.line_col(range.end());
    Range::new(
        Position::new(start.line - 1, start.col - 1),
        Position::new(end.line - 1, end.col - 1),
    )
}

// ── Go-to-definition ────────────────────────────────────────────────

/// Convert an LSP Position (0-based line, 0-based UTF-16 col) to a byte offset.
#[allow(clippy::cast_possible_truncation)]
fn lsp_position_to_offset(
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
    position: Position,
) -> Option<sass_parser::text_range::TextSize> {
    let line_start = line_index.line_start(position.line)? as usize;
    let remaining = &source[line_start..];
    let line_text = remaining.split('\n').next().unwrap_or(remaining);

    let target_utf16 = position.character;
    let mut byte_offset = 0usize;
    let mut utf16_offset = 0u32;

    for ch in line_text.chars() {
        if utf16_offset >= target_utf16 {
            break;
        }
        byte_offset += ch.len_utf8();
        utf16_offset += ch.len_utf16() as u32;
    }

    Some(sass_parser::text_range::TextSize::from(
        (line_start + byte_offset) as u32,
    ))
}

struct ReferenceInfo {
    namespace: Option<String>,
    name: String,
    kind: symbols::SymbolKind,
    range: TextRange,
}

fn find_reference_at_offset(
    root: &SyntaxNode,
    offset: sass_parser::text_range::TextSize,
) -> Option<ReferenceInfo> {
    let token = root.token_at_offset(offset).right_biased()?;

    for node in token.parent()?.ancestors() {
        match node.kind() {
            SyntaxKind::NAMESPACE_REF => {
                return extract_namespace_ref_info(&node);
            }
            SyntaxKind::VARIABLE_REF => {
                if node
                    .parent()
                    .is_some_and(|p| p.kind() == SyntaxKind::VARIABLE_DECL)
                {
                    return None;
                }
                let (name, range) = dollar_ident_name_range(&node)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Variable,
                    range,
                });
            }
            SyntaxKind::FUNCTION_CALL => {
                let (name, range) = ident_text_range_of(&node)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Function,
                    range,
                });
            }
            SyntaxKind::INCLUDE_RULE => {
                if node
                    .children()
                    .any(|c| c.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    return None;
                }
                let (name, range) = nth_ident_text_range_of(&node, 1)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Mixin,
                    range,
                });
            }
            SyntaxKind::EXTEND_RULE => {
                let (name, range) = percent_ident_name_range(&node)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Placeholder,
                    range,
                });
            }
            _ => {}
        }
    }
    None
}

fn extract_namespace_ref_info(node: &SyntaxNode) -> Option<ReferenceInfo> {
    let tokens: Vec<_> = node
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .collect();

    let namespace = tokens
        .iter()
        .find(|t| t.kind() == SyntaxKind::IDENT)?
        .text()
        .to_string();

    // ns.$var pattern: IDENT DOT DOLLAR IDENT
    if let Some(dollar) = tokens.iter().find(|t| t.kind() == SyntaxKind::DOLLAR) {
        let ident = tokens
            .iter()
            .skip_while(|t| t.kind() != SyntaxKind::DOLLAR)
            .find(|t| t.kind() == SyntaxKind::IDENT)?;
        let range = TextRange::new(dollar.text_range().start(), ident.text_range().end());
        return Some(ReferenceInfo {
            namespace: Some(namespace),
            name: ident.text().to_string(),
            kind: symbols::SymbolKind::Variable,
            range,
        });
    }

    // ns.func() pattern: has FUNCTION_CALL child
    if let Some(func_call) = node
        .children()
        .find(|c| c.kind() == SyntaxKind::FUNCTION_CALL)
    {
        let (name, range) = ident_text_range_of(&func_call)?;
        return Some(ReferenceInfo {
            namespace: Some(namespace),
            name,
            kind: symbols::SymbolKind::Function,
            range,
        });
    }

    // ns.mixin pattern: IDENT DOT IDENT (inside @include)
    let dot_pos = tokens.iter().position(|t| t.kind() == SyntaxKind::DOT)?;
    let ident = tokens[dot_pos + 1..]
        .iter()
        .find(|t| t.kind() == SyntaxKind::IDENT)?;

    let is_mixin = node
        .parent()
        .is_some_and(|p| p.kind() == SyntaxKind::INCLUDE_RULE);

    Some(ReferenceInfo {
        namespace: Some(namespace),
        name: ident.text().to_string(),
        kind: if is_mixin {
            symbols::SymbolKind::Mixin
        } else {
            symbols::SymbolKind::Function
        },
        range: ident.text_range(),
    })
}

/// Extract `$name` → (name, DOLLAR..IDENT range) from direct children.
fn dollar_ident_name_range(node: &SyntaxNode) -> Option<(String, TextRange)> {
    let mut dollar_start = None;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::DOLLAR => dollar_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if dollar_start.is_some() => {
                    let range = TextRange::new(dollar_start.unwrap(), token.text_range().end());
                    return Some((token.text().to_string(), range));
                }
                _ => {}
            }
        }
    }
    None
}

/// Extract first IDENT → (text, range).
fn ident_text_range_of(node: &SyntaxNode) -> Option<(String, TextRange)> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::IDENT)
        .map(|t| (t.text().to_string(), t.text_range()))
}

/// Extract nth IDENT → (text, range).
fn nth_ident_text_range_of(node: &SyntaxNode, n: usize) -> Option<(String, TextRange)> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|t| t.kind() == SyntaxKind::IDENT)
        .nth(n)
        .map(|t| (t.text().to_string(), t.text_range()))
}

/// Extract `%name` → (name, PERCENT..IDENT range) from direct children.
fn percent_ident_name_range(node: &SyntaxNode) -> Option<(String, TextRange)> {
    let mut pct_start = None;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::PERCENT => pct_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if pct_start.is_some() => {
                    let range = TextRange::new(pct_start.unwrap(), token.text_range().end());
                    return Some((token.text().to_string(), range));
                }
                _ => {}
            }
        }
    }
    None
}

/// For variables (`$name`) and placeholders (`%name`), strip the sigil
/// to get just the IDENT range. For functions/mixins, the range is already
/// just the IDENT.
fn name_only_range(kind: symbols::SymbolKind, range: TextRange) -> TextRange {
    match kind {
        symbols::SymbolKind::Variable | symbols::SymbolKind::Placeholder => {
            // Skip 1-byte sigil ($ or %)
            let start = range.start() + sass_parser::text_range::TextSize::from(1u32);
            if start < range.end() {
                TextRange::new(start, range.end())
            } else {
                range
            }
        }
        symbols::SymbolKind::Function | symbols::SymbolKind::Mixin => range,
    }
}

// ── Hover ───────────────────────────────────────────────────────────

fn find_definition_at_offset(
    symbols: &symbols::FileSymbols,
    offset: sass_parser::text_range::TextSize,
) -> Option<&symbols::Symbol> {
    symbols
        .definitions
        .iter()
        .find(|s| s.selection_range.contains(offset))
}

fn make_hover(sym: &symbols::Symbol, source_uri: Option<&Uri>) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format_hover_markdown(sym, source_uri),
        }),
        range: None,
    }
}

fn format_hover_markdown(sym: &symbols::Symbol, source_uri: Option<&Uri>) -> String {
    let signature = match sym.kind {
        symbols::SymbolKind::Variable => {
            if let Some(value) = &sym.value {
                format!("${}: {value}", sym.name)
            } else {
                format!("${}", sym.name)
            }
        }
        symbols::SymbolKind::Function => {
            let params = sym.params.as_deref().unwrap_or("()");
            format!("@function {}{params}", sym.name)
        }
        symbols::SymbolKind::Mixin => {
            let params = sym.params.as_deref().unwrap_or("");
            format!("@mixin {}{params}", sym.name)
        }
        symbols::SymbolKind::Placeholder => format!("%{}", sym.name),
    };

    let mut parts = vec![format!("```scss\n{signature}\n```")];

    if let Some(doc) = &sym.doc {
        parts.push(doc.clone());
    }

    if let Some(uri) = source_uri {
        if let Some(module) = builtins::builtin_name_from_uri(uri.as_str()) {
            parts.push(format!("Sass built-in (`sass:{module}`)"));
        } else if let Some(path) = uri.to_file_path() {
            if let Some(name) = path.file_name() {
                parts.push(format!("Defined in `{}`", name.to_string_lossy()));
            }
        }
    }

    parts.join("\n\n")
}

// ── Signature help ──────────────────────────────────────────────────

struct CallInfo {
    namespace: Option<String>,
    name: String,
    kind: symbols::SymbolKind,
    arg_list_start: sass_parser::text_range::TextSize,
}

fn find_call_at_offset(
    root: &SyntaxNode,
    offset: sass_parser::text_range::TextSize,
) -> Option<CallInfo> {
    let token = root.token_at_offset(offset).left_biased()?;

    for node in token.parent()?.ancestors() {
        match node.kind() {
            SyntaxKind::FUNCTION_CALL => {
                let arg_list = node.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)?;
                if !arg_list.text_range().contains(offset) {
                    continue;
                }

                // Check if inside a NAMESPACE_REF parent
                if let Some(ns_ref) = node
                    .parent()
                    .filter(|p| p.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    let ns_name = ns_ref
                        .children_with_tokens()
                        .filter_map(rowan::NodeOrToken::into_token)
                        .find(|t| t.kind() == SyntaxKind::IDENT)?
                        .text()
                        .to_string();
                    let func_name = ident_text_range_of(&node)?.0;
                    return Some(CallInfo {
                        namespace: Some(ns_name),
                        name: func_name,
                        kind: symbols::SymbolKind::Function,
                        arg_list_start: arg_list.text_range().start(),
                    });
                }

                let func_name = ident_text_range_of(&node)?.0;
                return Some(CallInfo {
                    namespace: None,
                    name: func_name,
                    kind: symbols::SymbolKind::Function,
                    arg_list_start: arg_list.text_range().start(),
                });
            }
            SyntaxKind::INCLUDE_RULE => {
                let arg_list = node.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)?;
                if !arg_list.text_range().contains(offset) {
                    continue;
                }

                // Check if has a NAMESPACE_REF child
                if let Some(ns_ref) = node
                    .children()
                    .find(|c| c.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    let tokens: Vec<_> = ns_ref
                        .children_with_tokens()
                        .filter_map(rowan::NodeOrToken::into_token)
                        .collect();
                    let ns_name = tokens
                        .iter()
                        .find(|t| t.kind() == SyntaxKind::IDENT)?
                        .text()
                        .to_string();
                    let dot_pos = tokens.iter().position(|t| t.kind() == SyntaxKind::DOT)?;
                    let mixin_name = tokens[dot_pos + 1..]
                        .iter()
                        .find(|t| t.kind() == SyntaxKind::IDENT)?
                        .text()
                        .to_string();
                    return Some(CallInfo {
                        namespace: Some(ns_name),
                        name: mixin_name,
                        kind: symbols::SymbolKind::Mixin,
                        arg_list_start: arg_list.text_range().start(),
                    });
                }

                let mixin_name = nth_ident_text_range_of(&node, 1)?.0;
                return Some(CallInfo {
                    namespace: None,
                    name: mixin_name,
                    kind: symbols::SymbolKind::Mixin,
                    arg_list_start: arg_list.text_range().start(),
                });
            }
            _ => {}
        }
    }
    None
}

#[allow(clippy::cast_possible_truncation)]
fn count_active_parameter(
    source: &str,
    call_info: &CallInfo,
    cursor: sass_parser::text_range::TextSize,
) -> u32 {
    let start = u32::from(call_info.arg_list_start) as usize;
    let cursor_pos = u32::from(cursor) as usize;
    if cursor_pos <= start {
        return 0;
    }

    let slice = &source[start..cursor_pos];

    // Count commas that are not inside nested parens/brackets
    let mut depth = 0u32;
    let mut commas = 0u32;
    for ch in slice.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 1 => commas += 1,
            _ => {}
        }
    }
    commas
}

fn build_signature_info(sym: &symbols::Symbol, params_text: &str) -> SignatureInformation {
    let label = match sym.kind {
        symbols::SymbolKind::Function => format!("@function {}{params_text}", sym.name),
        symbols::SymbolKind::Mixin => format!("@mixin {}{params_text}", sym.name),
        _ => {
            return SignatureInformation {
                label: sym.name.clone(),
                documentation: None,
                parameters: None,
                active_parameter: None,
            };
        }
    };

    let parameters = parse_param_labels(&label, params_text);

    let documentation = sym.doc.as_ref().map(|d| {
        tower_lsp_server::ls_types::Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d.clone(),
        })
    });

    SignatureInformation {
        label,
        documentation,
        parameters: Some(parameters),
        active_parameter: None,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn parse_param_labels(signature: &str, params_text: &str) -> Vec<ParameterInformation> {
    // Find the offset of params_text within the signature
    let Some(params_offset) = signature.find(params_text) else {
        return Vec::new();
    };

    // Strip outer parens
    let inner = params_text
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(params_text);

    if inner.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    // +1 for the opening paren
    let content_offset = params_offset + 1;

    // Split by commas at depth 0 (handle nested parens in defaults)
    let mut depth = 0u32;
    let mut segment_start = 0;

    for (i, ch) in inner.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let param = inner[segment_start..i].trim();
                if !param.is_empty() {
                    let abs_start = content_offset + segment_start;
                    let abs_end = content_offset + segment_start + param.len();
                    result.push(ParameterInformation {
                        label: ParameterLabel::LabelOffsets([abs_start as u32, abs_end as u32]),
                        documentation: None,
                    });
                }
                segment_start = i + 1;
                // Skip whitespace after comma
                for (j, c) in inner[segment_start..].char_indices() {
                    if c != ' ' {
                        segment_start += j;
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    // Last segment
    let param = inner[segment_start..].trim();
    if !param.is_empty() {
        let abs_start = content_offset + segment_start;
        let abs_end = content_offset + segment_start + param.len();
        result.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([abs_start as u32, abs_end as u32]),
            documentation: None,
        });
    }

    result
}

// ── Workspace symbol search ─────────────────────────────────────────

fn fuzzy_match(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let name_lower = name.to_lowercase();
    let mut name_chars = name_lower.chars();
    for qch in query.chars() {
        if name_chars.find(|&c| c == qch).is_none() {
            return false;
        }
    }
    true
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Send a JSON-RPC message with Content-Length framing.
    async fn send_msg(writer: &mut (impl AsyncWriteExt + Unpin), msg: &Value) {
        let body = serde_json::to_string(msg).unwrap();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        writer.write_all(header.as_bytes()).await.unwrap();
        writer.write_all(body.as_bytes()).await.unwrap();
        writer.flush().await.unwrap();
    }

    /// Read one JSON-RPC message from the stream (blocking).
    async fn recv_msg(reader: &mut (impl AsyncReadExt + Unpin)) -> Value {
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

    /// Spawn the LSP server on in-memory duplex streams, return client-side handles.
    fn spawn_server() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
        let (client_read, server_write) = tokio::io::duplex(1024 * 64);
        let (server_read, client_write) = tokio::io::duplex(1024 * 64);

        let (service, socket) = LspService::new(|client| {
            let documents = Arc::new(DashMap::new());
            let module_graph = Arc::new(workspace::ModuleGraph::new());
            let (task_tx, task_rx) = mpsc::unbounded_channel();
            tokio::spawn(run_worker(
                task_rx,
                client.clone(),
                Arc::clone(&documents),
                Arc::clone(&module_graph),
            ));
            Backend {
                client,
                documents,
                module_graph,
                task_tx,
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
        let resp = recv_msg(reader).await;

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
        assert_eq!(caps["textDocumentSync"], 1);

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
                "macro",
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
        let notif = recv_msg(&mut reader).await;
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

        let notif = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _ = recv_msg(&mut reader).await; // open diagnostics

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

        let notif = recv_msg(&mut reader).await;
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
        let _ = recv_msg(&mut reader).await;

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
        let notif = recv_msg(&mut reader).await;
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

        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(content.contains("$color"), "hover should show variable name");
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(content.contains("$primary"), "hover on def should show name");
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
        assert!(resp["result"].is_null(), "hover on empty space should be null");
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
        let content = resp["result"]["contents"]["value"].as_str().unwrap();
        assert!(content.contains("primary color"), "hover should show doc comment");
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
        assert!(resp["result"].is_null());
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let result = super::parse_param_labels(&sig, params);
        assert_eq!(result.len(), 2);
        // "$a" starts at offset 12 (after "@function f("), ends at 14
        if let ParameterLabel::LabelOffsets([s, e]) = &result[0].label {
            assert_eq!(&sig[*s as usize..*e as usize], "$a");
        } else {
            panic!("expected LabelOffsets");
        }
        // "$b: 1px" starts at 16 (after "$a, "), ends at 23
        if let ParameterLabel::LabelOffsets([s, e]) = &result[1].label {
            assert_eq!(&sig[*s as usize..*e as usize], "$b: 1px");
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
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
        let _diag = recv_msg(&mut reader).await;

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

        let resp = recv_msg(&mut reader).await;
        assert!(resp["result"].is_null(), "no match should return null");
    }

    #[test]
    fn fuzzy_match_basics() {
        assert!(super::fuzzy_match("responsive-grid", "rg"));
        assert!(super::fuzzy_match("primary", "pry"));
        assert!(super::fuzzy_match("Button", "btn"));
        assert!(!super::fuzzy_match("simple", "rg"));
        assert!(super::fuzzy_match("anything", ""));
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
        let resp = recv_msg(reader).await;

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
        let module_graph = Arc::new(workspace::ModuleGraph::new());
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_worker(
            task_rx,
            client.clone(),
            Arc::clone(&documents),
            Arc::clone(&module_graph),
        ));
        Backend {
            client,
            documents,
            module_graph,
            task_tx,
        }
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
