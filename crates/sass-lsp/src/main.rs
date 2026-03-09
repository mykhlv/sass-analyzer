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
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, InitializedParams, Position,
    Range, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
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
    Parse { uri: Uri, version: i32, text: String },
    Close { uri: Uri },
}

#[allow(dead_code)]
struct Backend {
    client: Client,
    documents: Arc<DashMap<Uri, DocumentState>>,
    task_tx: mpsc::UnboundedSender<Task>,
}

struct DocumentState {
    version: i32,
    text: String,
    green: rowan::GreenNode,
    #[allow(dead_code)]
    errors: Vec<(String, TextRange)>,
    line_index: sass_parser::line_index::LineIndex,
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
        let (line, col) =
            byte_to_lsp_pos(source, line_index, sass_parser::text_range::TextSize::from(tok.start));

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

                    let is_current = documents
                        .get(&uri)
                        .is_none_or(|state| state.version <= version);

                    documents.insert(
                        uri.clone(),
                        DocumentState { version, text, green, errors, line_index },
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
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_worker(task_rx, client.clone(), Arc::clone(&documents)));
        Backend { client, documents, task_tx }
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
