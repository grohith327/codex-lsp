use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp_server::Client;
use tower_lsp_server::LanguageServer;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;

use crate::completion;
use crate::diagnostics;
use crate::document::DocumentStore;
use crate::file_search::FffFileSearch;
use crate::registry::Registry;

pub struct Backend {
    client: Client,
    docs: DocumentStore,
    registry: Arc<RwLock<Registry>>,
    workspace_roots: Arc<RwLock<Vec<PathBuf>>>,
    file_search: Arc<FffFileSearch>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            docs: DocumentStore::new(),
            registry: Arc::new(RwLock::new(Registry::default())),
            workspace_roots: Arc::new(RwLock::new(Vec::new())),
            file_search: Arc::new(FffFileSearch::default()),
        }
    }

    async fn validate(&self, uri: Uri, version: Option<i32>) {
        let Some(rope) = self.docs.rope(&uri) else {
            return;
        };
        let registry = self.registry.read().await.clone();
        let doc_dir = uri_to_dir(&uri);
        let diags = diagnostics::compute(&rope, &registry, doc_dir.as_deref()).await;
        self.client.publish_diagnostics(uri, diags, version).await;
    }

    async fn search_root(&self, uri: &Uri) -> Option<PathBuf> {
        if let Some(dir) = uri_to_dir(uri) {
            return Some(dir);
        }
        self.workspace_roots.read().await.first().cloned()
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let mut roots: Vec<PathBuf> = Vec::new();
        if let Some(folders) = params.workspace_folders {
            for folder in folders {
                if let Some(path) = uri_to_path(&folder.uri) {
                    roots.push(path);
                }
            }
        }
        #[allow(deprecated)]
        if roots.is_empty()
            && let Some(uri) = params.root_uri
            && let Some(path) = uri_to_path(&uri)
        {
            roots.push(path);
        }

        *self.workspace_roots.write().await = roots.clone();
        *self.registry.write().await = Registry::load(&roots);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "codex-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "@".to_string(),
                        "/".to_string(),
                        "$".to_string(),
                    ]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let n = self.registry.read().await;
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "codex-lsp ready ({} prompts, {} skills)",
                    n.prompts.len(),
                    n.skills.len()
                ),
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.docs.upsert(doc.uri.clone(), &doc.text);
        self.validate(doc.uri, Some(doc.version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // FULL sync: the last change carries the entire document text.
        if let Some(change) = params.content_changes.into_iter().next_back() {
            let uri = params.text_document.uri;
            self.docs.upsert(uri.clone(), &change.text);
            self.validate(uri, Some(params.text_document.version)).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.docs.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let Some(rope) = self.docs.rope(&uri) else {
            return Ok(None);
        };
        let registry = self.registry.read().await.clone();
        let search_root = self.search_root(&uri).await;
        Ok(completion::complete(
            &rope,
            pos,
            &registry,
            search_root.as_deref(),
            &self.file_search,
        )
        .await)
    }
}

fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    uri.to_file_path().map(|p| p.to_path_buf())
}

fn uri_to_dir(uri: &Uri) -> Option<PathBuf> {
    uri_to_path(uri).and_then(|p| p.parent().map(Path::to_path_buf))
}
