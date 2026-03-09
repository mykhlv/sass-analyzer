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
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, InitializeParams,
    InitializeResult, InitializedParams, OneOf, Position, Range, SemanticToken,
    SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
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
                        module_graph.remove_file(&uri);
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
                        &text,
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
                document_symbol_provider: Some(OneOf::Left(true)),
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
