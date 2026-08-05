use lsp_max::{LspService, Server};
use val_pddl_lsp::Backend;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    let _ = Server::new(stdin, stdout, socket).serve(service).await;
}
