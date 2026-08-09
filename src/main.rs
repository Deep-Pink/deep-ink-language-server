use deep_ink_language_server::db::start_database_service;
use deep_ink_language_server::{
    DbMessage, DiagnosticsMessage, OpenInkDocument, UpdateInkDocument, UpdateRange,
};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tower_lsp_server::jsonrpc::{Error, Result};
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    db_message_sender: Sender<DbMessage>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF8),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: None,
                        inter_file_dependencies: true,
                        workspace_diagnostics: true,
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                    },
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "deep_ink_language_server".to_owned(),
                version: Some("v0.1.0".to_owned()),
            }),
            offset_encoding: None,
        })
    }

    async fn workspace_diagnostic(
        &self,
        _params: WorkspaceDiagnosticParams,
    ) -> Result<WorkspaceDiagnosticReportResult> {
        let (sender, mut receiver) = channel(64);
        let _ = self
            .db_message_sender
            .send(DbMessage::RequestDiagnostics(sender))
            .await;
        if let Some(DiagnosticsMessage(diagnostics)) = receiver.recv().await {
            self.client
                .log_message(MessageType::INFO, "Received Diagnostics!")
                .await;
            let mut items: Vec<WorkspaceDocumentDiagnosticReport> = vec![];
            for ((uri, version), diagnostics_list) in diagnostics.into_iter() {
                items.push(WorkspaceDocumentDiagnosticReport::Full(
                    WorkspaceFullDocumentDiagnosticReport {
                        uri,
                        version: Some(version as i64),
                        full_document_diagnostic_report: FullDocumentDiagnosticReport {
                            result_id: None,
                            items: diagnostics_list,
                        },
                    },
                ));
            }
            return Result::Ok(WorkspaceDiagnosticReportResult::Report(
                WorkspaceDiagnosticReport { items },
            ));
        };
        return Result::Err(Error::internal_error());
    }

    async fn did_change_workspace_folders(&self, _params: DidChangeWorkspaceFoldersParams) {
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
        let (sender, receiver) = channel(64);

        let _ = self
            .db_message_sender
            .send(DbMessage::Open(
                OpenInkDocument {
                    uri: params.text_document.uri.clone(),
                    version: params.text_document.version,
                    contents: params.text_document.text.clone(),
                },
                sender,
            ))
            .await;

        let _ = self.wait_and_publish_diagnostics(receiver).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file changed!")
            .await;
        let (sender, receiver) = channel(64);
        let changes: Vec<UpdateInkDocument> = params
            .content_changes
            .into_iter()
            .map(|mut x| {
                if let Some(range) = x.range.take() {
                    UpdateInkDocument {
                        uri: params.text_document.uri.clone(),
                        version: params.text_document.version,
                        range: UpdateRange::Range(range),
                        new_text: x.text,
                    }
                } else {
                    UpdateInkDocument {
                        uri: params.text_document.uri.clone(),
                        version: params.text_document.version,
                        range: UpdateRange::All,
                        new_text: x.text,
                    }
                }
            })
            .collect();
        match self
            .db_message_sender
            .send(DbMessage::Update(changes, sender))
            .await
        {
            Ok(_) => {
                self.wait_and_publish_diagnostics(receiver).await;
            }
            Err(err) => {
                self.client
                    .log_message(MessageType::ERROR, err.to_string())
                    .await;
            }
        };
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        let (sender, receiver) = channel(64);
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
        let _ = self
            .db_message_sender
            .send(DbMessage::RequestDiagnostics(sender))
            .await;
        let _ = self.wait_and_publish_diagnostics(receiver).await;
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }
}

impl Backend {
    async fn wait_and_publish_diagnostics(&self, mut receiver: Receiver<DiagnosticsMessage>) {
        match receiver.recv().await {
            Some(DiagnosticsMessage(diagnostics)) => {
                self.client
                    .log_message(MessageType::INFO, "Received Diagnostics!")
                    .await;
                for ((uri, version), diagnostics_list) in diagnostics.into_iter() {
                    self.client
                        .log_message(
                            MessageType::INFO,
                            format!(
                                "sending through diagnostics for {} with length {}",
                                uri.to_string(),
                                diagnostics_list.len()
                            ),
                        )
                        .await;
                    self.client
                        .publish_diagnostics(uri, diagnostics_list, Some(version))
                        .await;
                }
            }
            None => {
                self.client
                    .log_message(MessageType::ERROR, "Failed to receive message")
                    .await;
            }
        };
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (db_message_sender, db_message_receiver) = channel(32);
    let db_service = start_database_service(db_message_receiver);
    let (lsp_service, socket) = LspService::new(|client| Backend {
        client,
        db_message_sender,
    });
    let server = Server::new(stdin, stdout, socket).serve(lsp_service);
    db_service.await;
    server.await;
}
