use anyhow::Result;
use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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

    let addr = format!("127.0.0.1:{}", port);
    info!("Starting WebSocket server on {}", addr);
    
    // Create lock file for CLI discovery
    create_lock_file(port).await?;
    
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on {}", addr);
    
    // Shared state for managing connections
    let server_state = Arc::new(ServerState::new());
    
    // Setup graceful shutdown
    let shutdown_port = port;
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            error!("Failed to listen for shutdown signal: {}", e);
        } else {
            info!("Shutdown signal received, cleaning up...");
            if let Err(e) = cleanup_lock_file(shutdown_port).await {
                error!("Failed to cleanup lock file: {}", e);
            }
            std::process::exit(0);
        }
    });
    
    while let Ok((stream, addr)) = listener.accept().await {
        info!("New connection from {}", addr);
        let state = Arc::clone(&server_state);
        tokio::spawn(handle_connection(stream, addr, state));
    }
    
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

// WebSocket server state and message types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeCodeMessage {
    id: String,
    #[serde(rename = "type")]
    message_type: String,
    data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeCodeResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    data: Value,
    error: Option<String>,
}

#[derive(Debug)]
struct ServerState {
    connections: Arc<RwLock<HashMap<String, String>>>,
    message_handlers: Arc<RwLock<HashMap<String, broadcast::Sender<ClaudeCodeMessage>>>>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// Lock file management
async fn create_lock_file(port: u16) -> Result<()> {
    let lock_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude")
        .join("ide");
    
    tokio::fs::create_dir_all(&lock_dir).await?;
    
    let lock_file = lock_dir.join(format!("{}.lock", port));
    let lock_data = serde_json::json!({
        "port": port,
        "pid": std::process::id(),
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    tokio::fs::write(&lock_file, serde_json::to_string_pretty(&lock_data)?).await?;
    info!("Lock file created at {}", lock_file.display());
    
    Ok(())
}

// WebSocket connection handler
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<ServerState>,
) -> Result<()> {
    let ws_stream = accept_async(stream).await?;
    let connection_id = Uuid::new_v4().to_string();
    
    info!("WebSocket connection established: {} ({})", connection_id, addr);
    
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    // Store connection ID
    {
        let mut connections = state.connections.write().await;
        connections.insert(connection_id.clone(), addr.to_string());
    }
    
    // Handle incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received message: {}", text);
                
                match serde_json::from_str::<ClaudeCodeMessage>(&text) {
                    Ok(message) => {
                        let response = handle_claude_code_message(message, &state).await;
                        let response_text = serde_json::to_string(&response)?;
                        
                        if let Err(e) = ws_sender.send(Message::Text(response_text)).await {
                            error!("Failed to send response: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse message: {}", e);
                        let error_response = ClaudeCodeResponse {
                            id: Uuid::new_v4().to_string(),
                            response_type: "error".to_string(),
                            data: Value::Null,
                            error: Some(format!("Invalid message format: {}", e)),
                        };
                        
                        if let Ok(response_text) = serde_json::to_string(&error_response) {
                            let _ = ws_sender.send(Message::Text(response_text)).await;
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed: {}", connection_id);
                break;
            }
            Ok(Message::Ping(payload)) => {
                if let Err(e) = ws_sender.send(Message::Pong(payload)).await {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {
                // Ignore pong messages
            }
            Ok(Message::Binary(_)) => {
                warn!("Received binary message, ignoring");
            }
            Ok(Message::Frame(_)) => {
                // Handle frame messages (typically handled internally)
                debug!("Received frame message");
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }
    
    // Clean up connection
    {
        let mut connections = state.connections.write().await;
        connections.remove(&connection_id);
    }
    
    info!("WebSocket connection closed: {} ({})", connection_id, addr);
    Ok(())
}

// Handle Claude Code protocol messages
async fn handle_claude_code_message(
    message: ClaudeCodeMessage,
    _state: &Arc<ServerState>,
) -> ClaudeCodeResponse {
    match message.message_type.as_str() {
        "ping" => ClaudeCodeResponse {
            id: message.id,
            response_type: "pong".to_string(),
            data: message.data,
            error: None,
        },
        "get_capabilities" => ClaudeCodeResponse {
            id: message.id,
            response_type: "capabilities".to_string(),
            data: serde_json::json!({
                "version": "0.1.0",
                "features": ["lsp", "websocket", "file_operations", "code_execution"],
                "supported_languages": ["rust", "javascript", "typescript", "python", "go", "java", "c", "cpp"]
            }),
            error: None,
        },
        "execute_command" => {
            // Handle command execution (like running tests, building, etc.)
            let command = message.data.get("command").and_then(|v| v.as_str());
            let args = message.data.get("args").and_then(|v| v.as_array());
            
            match command {
                Some(cmd) => {
                    info!("Executing command: {} with args: {:?}", cmd, args);
                    
                    // For now, just echo the command back
                    // In a full implementation, this would execute the command
                    ClaudeCodeResponse {
                        id: message.id,
                        response_type: "command_result".to_string(),
                        data: serde_json::json!({
                            "command": cmd,
                            "args": args,
                            "status": "success",
                            "output": "Command executed successfully (mock)"
                        }),
                        error: None,
                    }
                }
                None => ClaudeCodeResponse {
                    id: message.id,
                    response_type: "error".to_string(),
                    data: Value::Null,
                    error: Some("Missing command in execute_command message".to_string()),
                },
            }
        }
        "get_file" => {
            // Handle file read operations
            let path = message.data.get("path").and_then(|v| v.as_str());
            
            match path {
                Some(file_path) => {
                    match tokio::fs::read_to_string(file_path).await {
                        Ok(content) => ClaudeCodeResponse {
                            id: message.id,
                            response_type: "file_content".to_string(),
                            data: serde_json::json!({
                                "path": file_path,
                                "content": content
                            }),
                            error: None,
                        },
                        Err(e) => ClaudeCodeResponse {
                            id: message.id,
                            response_type: "error".to_string(),
                            data: Value::Null,
                            error: Some(format!("Failed to read file: {}", e)),
                        },
                    }
                }
                None => ClaudeCodeResponse {
                    id: message.id,
                    response_type: "error".to_string(),
                    data: Value::Null,
                    error: Some("Missing path in get_file message".to_string()),
                },
            }
        }
        "write_file" => {
            // Handle file write operations
            let path = message.data.get("path").and_then(|v| v.as_str());
            let content = message.data.get("content").and_then(|v| v.as_str());
            
            match (path, content) {
                (Some(file_path), Some(file_content)) => {
                    match tokio::fs::write(file_path, file_content).await {
                        Ok(_) => ClaudeCodeResponse {
                            id: message.id,
                            response_type: "file_written".to_string(),
                            data: serde_json::json!({
                                "path": file_path,
                                "size": file_content.len()
                            }),
                            error: None,
                        },
                        Err(e) => ClaudeCodeResponse {
                            id: message.id,
                            response_type: "error".to_string(),
                            data: Value::Null,
                            error: Some(format!("Failed to write file: {}", e)),
                        },
                    }
                }
                _ => ClaudeCodeResponse {
                    id: message.id,
                    response_type: "error".to_string(),
                    data: Value::Null,
                    error: Some("Missing path or content in write_file message".to_string()),
                },
            }
        }
        _ => ClaudeCodeResponse {
            id: message.id,
            response_type: "error".to_string(),
            data: Value::Null,
            error: Some(format!("Unknown message type: {}", message.message_type)),
        },
    }
}

// Cleanup lock file on shutdown
async fn cleanup_lock_file(port: u16) -> Result<()> {
    let lock_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude")
        .join("ide");
    
    let lock_file = lock_dir.join(format!("{}.lock", port));
    
    if lock_file.exists() {
        tokio::fs::remove_file(&lock_file).await?;
        info!("Lock file cleaned up: {}", lock_file.display());
    }
    
    Ok(())
}
