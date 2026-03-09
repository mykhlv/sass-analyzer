use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, Position, Range,
    SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    documents: Arc<DashMap<Uri, DocumentState>>,
}

#[allow(dead_code)]
struct DocumentState {
    version: i32,
    text: String,
    green: rowan::GreenNode,
    errors: Vec<(String, sass_parser::text_range::TextRange)>,
    line_index: sass_parser::line_index::LineIndex,
}

const MAX_FILE_SIZE: usize = 2_000_000;

fn parse_document(text: &str) -> Option<(rowan::GreenNode, Vec<(String, sass_parser::text_range::TextRange)>)> {
    std::panic::catch_unwind(AssertUnwindSafe(|| sass_parser::parse(text))).ok()
}

impl Backend {
    fn reparse_and_publish(&self, uri: Uri, version: i32, text: String) {
        let Some((green, errors)) = parse_document(&text) else {
            tracing::error!("parser panic for {uri:?}");
            return;
        };

        let line_index = sass_parser::line_index::LineIndex::new(&text);
        let diagnostics = errors_to_diagnostics(&errors, &line_index);

        self.documents.insert(
            uri.clone(),
            DocumentState { version, text, green, errors, line_index },
        );

        let client = self.client.clone();
        tokio::spawn(async move {
            client.publish_diagnostics(uri, diagnostics, Some(version)).await;
        });
    }
}

fn errors_to_diagnostics(
    errors: &[(String, sass_parser::text_range::TextRange)],
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

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        if doc.text.len() > MAX_FILE_SIZE {
            return;
        }
        self.reparse_and_publish(doc.uri, doc.version, doc.text);
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
        self.reparse_and_publish(uri, version, change.text);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        let client = self.client.clone();
        tokio::spawn(async move {
            client.publish_diagnostics(uri, vec![], None).await;
        });
    }
}

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

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Arc::new(DashMap::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
