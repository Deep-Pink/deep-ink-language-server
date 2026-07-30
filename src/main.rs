use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: todo!(),
                text_document_sync: todo!(),
                notebook_document_sync: todo!(),
                selection_range_provider: todo!(),
                hover_provider: todo!(),
                completion_provider: todo!(),
                signature_help_provider: todo!(),
                definition_provider: todo!(),
                type_definition_provider: todo!(),
                implementation_provider: todo!(),
                references_provider: todo!(),
                document_highlight_provider: todo!(),
                document_symbol_provider: todo!(),
                workspace_symbol_provider: todo!(),
                code_action_provider: todo!(),
                code_lens_provider: todo!(),
                document_formatting_provider: todo!(),
                document_range_formatting_provider: todo!(),
                document_on_type_formatting_provider: todo!(),
                rename_provider: todo!(),
                document_link_provider: todo!(),
                color_provider: todo!(),
                folding_range_provider: todo!(),
                declaration_provider: todo!(),
                execute_command_provider: todo!(),
                workspace: todo!(),
                call_hierarchy_provider: todo!(),
                semantic_tokens_provider: todo!(),
                moniker_provider: todo!(),
                linked_editing_range_provider: todo!(),
                inline_value_provider: todo!(),
                inlay_hint_provider: todo!(),
                diagnostic_provider: todo!(),
                inline_completion_provider: todo!(),
                ..Default::default()
            },
            server_info: todo!(),
            offset_encoding: todo!(),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
