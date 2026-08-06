use deep_ink_language_server::db::LspDb;
use deep_ink_language_server::{DbMessage, LspMessage, OpenInkDocument, UpdateInkDocument};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    db_message_sender: Sender<DbMessage>,
    lsp_message_receiver: Receiver<LspMessage>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: None,
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "deep_ink_language_server".to_owned(),
                version: Some("v0.1.0".to_owned()),
            }),
            offset_encoding: None,
        })
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "did change workspace folders!")
            .await;
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        let _ = self
            .db_message_sender
            .send(DbMessage::Open(OpenInkDocument {
                uri: params.text_document.uri.clone(),
                version: params.text_document.version,
                contents: params.text_document.text.clone(),
            }))
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file changed!")
            .await;
        let mut changes: Vec<UpdateInkDocument> = vec![];
        for change in params.content_changes {
            changes.push(UpdateInkDocument {
                uri: params.text_document.uri.clone(),
                version: params.text_document.version,
                range: change.range.clone(),
                new_text: change.text.clone(),
            });
        }
        let _ = self
            .db_message_sender
            .send(DbMessage::Update(changes))
            .await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }
    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (db_message_sender, db_message_receiver) = channel(32);
    let (lsp_message_sender, lsp_message_receiver) = channel(32);
    let join_handle = LspDb::start_database_service(db_message_receiver, lsp_message_sender);
    let (service, socket) = LspService::new(|client| Backend {
        client,
        db_message_sender,
        lsp_message_receiver,
    });
    Server::new(stdin, stdout, socket).serve(service).await;
    join_handle.await.expect("Expected join");
}
