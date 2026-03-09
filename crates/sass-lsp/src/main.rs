use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::mpsc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, InitializedParams, Position,
    Range, SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

const MAX_FILE_SIZE: usize = 2_000_000;
const DEBOUNCE_MS: u64 = 50;

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

#[allow(dead_code)]
struct DocumentState {
    version: i32,
    text: String,
    green: rowan::GreenNode,
    errors: Vec<(String, sass_parser::text_range::TextRange)>,
    line_index: sass_parser::line_index::LineIndex,
}

fn parse_document(
    text: &str,
) -> Option<(rowan::GreenNode, Vec<(String, sass_parser::text_range::TextRange)>)> {
    std::panic::catch_unwind(AssertUnwindSafe(|| sass_parser::parse(text))).ok()
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

                    // Stale version check: only publish if version is still current
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

    let (service, socket) = LspService::new(|client| {
        let documents = Arc::new(DashMap::new());
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_worker(task_rx, client.clone(), Arc::clone(&documents)));
        Backend { client, documents, task_tx }
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
