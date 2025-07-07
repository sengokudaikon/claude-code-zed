use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "claude-code-server")]
#[command(about = "Claude Code Server - WebSocket and LSP server for Claude Code integration")]
struct Cli {
    #[command(subcommand)]
    mode: Option<Mode>,

    /// Enable debug logging
    #[arg(long, short)]
    debug: bool,

    /// Worktree root path (for LSP mode)
    #[arg(long)]
    worktree: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Mode {
    /// Run as LSP server for Zed extension communication
    Lsp {
        /// Worktree root path
        #[arg(long)]
        worktree: Option<PathBuf>,
    },
    /// Run as standalone WebSocket server for Claude Code CLI
    Websocket {
        /// WebSocket server port (random if not specified)
        #[arg(long, short)]
        port: Option<u16>,
    },
    /// Run both LSP and WebSocket servers
    Hybrid {
        /// WebSocket server port (random if not specified)
        #[arg(long, short)]
        port: Option<u16>,
        /// Worktree root path
        #[arg(long)]
        worktree: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Claude Code Server starting...");

    match cli.mode {
        Some(Mode::Lsp { worktree }) => {
            let worktree_path = cli.worktree.or(worktree);
            run_lsp_server(worktree_path).await
        }
        Some(Mode::Websocket { port }) => run_websocket_server(port).await,
        Some(Mode::Hybrid { port, worktree }) => {
            let worktree_path = cli.worktree.or(worktree);
            run_hybrid_server(port, worktree_path).await
        }
        None => {
            // Default mode: try to detect what we should run based on arguments
            if cli.worktree.is_some() {
                info!("No mode specified but worktree provided, running LSP mode...");
                run_lsp_server(cli.worktree).await
            } else {
                info!("No mode specified, running in hybrid mode...");
                run_hybrid_server(None, cli.worktree).await
            }
        }
    }
}

async fn run_lsp_server(worktree: Option<PathBuf>) -> Result<()> {
    info!("Starting LSP server mode");
    if let Some(path) = &worktree {
        info!("Worktree path: {}", path.display());
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(|client| ClaudeCodeLanguageServer::new(client, worktree));
    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}

async fn run_websocket_server(port: Option<u16>) -> Result<()> {
    let port = port.unwrap_or_else(|| {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener
            .local_addr()
            .expect("Failed to get local addr")
            .port()
    });

    info!("Starting WebSocket server on port {}", port);
    warn!("WebSocket server not yet implemented");

    // TODO: Implement WebSocket server using tokio-tungstenite
    // This will handle Claude Code CLI connections
    // and implement the Claude Code protocol

    Ok(())
}

async fn run_hybrid_server(port: Option<u16>, worktree: Option<PathBuf>) -> Result<()> {
    info!("Starting hybrid server (LSP + WebSocket)");
    if let Some(path) = &worktree {
        info!("Worktree path: {}", path.display());
    }

    // In hybrid mode, we run both servers
    let websocket_handle = tokio::spawn(run_websocket_server(port));
    let lsp_handle = tokio::spawn(run_lsp_server(worktree));

    // Wait for either to complete (or fail)
    tokio::select! {
        result = websocket_handle => {
            match result {
                Ok(Ok(())) => info!("WebSocket server completed"),
                Ok(Err(e)) => error!("WebSocket server error: {}", e),
                Err(e) => error!("WebSocket server task panicked: {}", e),
            }
        }
        result = lsp_handle => {
            match result {
                Ok(Ok(())) => info!("LSP server completed"),
                Ok(Err(e)) => error!("LSP server error: {}", e),
                Err(e) => error!("LSP server task panicked: {}", e),
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct ClaudeCodeLanguageServer {
    client: Client,
    worktree: Option<PathBuf>,
}

impl ClaudeCodeLanguageServer {
    fn new(client: Client, worktree: Option<PathBuf>) -> Self {
        Self { client, worktree }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for ClaudeCodeLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        info!("LSP Server initializing...");
        if let Some(workspace_folders) = &params.workspace_folders {
            for folder in workspace_folders {
                info!("Workspace folder: {}", folder.uri);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec!["@".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "claude-code.explain".to_string(),
                        "claude-code.improve".to_string(),
                        "claude-code.fix".to_string(),
                    ],
                    work_done_progress_options: Default::default(),
                }),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "Claude Code Language Server".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("Claude Code LSP server initialized!");

        self.client
            .log_message(MessageType::INFO, "Claude Code Language Server is ready!")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        info!("LSP Server shutting down...");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        info!("Document opened: {}", params.text_document.uri);

        self.client
            .log_message(
                MessageType::INFO,
                format!("Opened document: {}", params.text_document.uri),
            )
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        info!("Document changed: {}", params.text_document.uri);
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        info!("Document saved: {}", params.text_document.uri);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        info!("Document closed: {}", params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let position = params.text_document_position_params.position;
        info!(
            "Hover requested at {}:{}",
            position.line, position.character
        );

        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(
                "Claude Code: AI-powered coding assistance available here".to_string(),
            )),
            range: None,
        }))
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let position = params.text_document_position.position;
        info!(
            "Completion requested at {}:{}",
            position.line, position.character
        );

        let completions = vec![
            CompletionItem {
                label: "@claude explain".to_string(),
                kind: Some(CompletionItemKind::TEXT),
                detail: Some("Explain this code with Claude".to_string()),
                documentation: Some(Documentation::String(
                    "Ask Claude to explain the selected code or current context".to_string(),
                )),
                insert_text: Some("@claude explain".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "@claude improve".to_string(),
                kind: Some(CompletionItemKind::TEXT),
                detail: Some("Improve this code with Claude".to_string()),
                documentation: Some(Documentation::String(
                    "Ask Claude to suggest improvements for the selected code".to_string(),
                )),
                insert_text: Some("@claude improve".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "@claude fix".to_string(),
                kind: Some(CompletionItemKind::TEXT),
                detail: Some("Fix issues in this code with Claude".to_string()),
                documentation: Some(Documentation::String(
                    "Ask Claude to identify and fix issues in the selected code".to_string(),
                )),
                insert_text: Some("@claude fix".to_string()),
                ..Default::default()
            },
        ];

        Ok(Some(CompletionResponse::Array(completions)))
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        info!("Code action requested for range: {:?}", params.range);

        let actions = vec![CodeActionOrCommand::CodeAction(CodeAction {
            title: "Explain with Claude".to_string(),
            kind: Some(CodeActionKind::REFACTOR),
            diagnostics: None,
            edit: None,
            command: None,
            is_preferred: Some(false),
            disabled: None,
            data: Some(serde_json::json!({
                "action": "explain",
                "uri": params.text_document.uri,
                "range": params.range
            })),
        })];

        Ok(Some(actions))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<Value>> {
        info!("Execute command: {}", params.command);

        match params.command.as_str() {
            "claude-code.explain" => {
                self.client
                    .show_message(
                        MessageType::INFO,
                        "Claude Code: Explain command executed (not yet implemented)",
                    )
                    .await;
            }
            "claude-code.improve" => {
                self.client
                    .show_message(
                        MessageType::INFO,
                        "Claude Code: Improve command executed (not yet implemented)",
                    )
                    .await;
            }
            "claude-code.fix" => {
                self.client
                    .show_message(
                        MessageType::INFO,
                        "Claude Code: Fix command executed (not yet implemented)",
                    )
                    .await;
            }
            _ => {
                self.client
                    .show_message(
                        MessageType::WARNING,
                        format!("Unknown command: {}", params.command),
                    )
                    .await;
            }
        }

        Ok(None)
    }
}
