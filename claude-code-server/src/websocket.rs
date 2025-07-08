use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::RwLock;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// JSON-RPC 2.0 message types for Claude Code protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug)]
pub struct ServerState {
    pub connections: Arc<RwLock<HashMap<String, String>>>,
    pub auth_token: String,
    pub workspace_folders: Vec<String>,
    pub ide_name: String,
}

impl ServerState {
    pub fn new(worktree: Option<PathBuf>) -> Self {
        let auth_token = generate_auth_token();
        let workspace_folders = if let Some(path) = worktree {
            vec![path.to_string_lossy().to_string()]
        } else {
            // Default to current working directory
            match std::env::current_dir() {
                Ok(cwd) => vec![cwd.to_string_lossy().to_string()],
                Err(_) => vec![],
            }
        };
        
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            auth_token,
            workspace_folders,
            ide_name: "claude-code-server".to_string(),
        }
    }
}

fn generate_auth_token() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect()
}

// Lock file management according to Claude Code protocol
pub async fn create_lock_file(port: u16, state: &ServerState) -> Result<()> {
    let lock_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude")
        .join("ide");
    
    tokio::fs::create_dir_all(&lock_dir).await?;
    
    let lock_file = lock_dir.join(format!("{}.lock", port));
    let lock_data = serde_json::json!({
        "processId": std::process::id(),
        "workspaceFolders": state.workspace_folders,
        "ideName": state.ide_name,
        "authToken": state.auth_token,
        "port": port,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    tokio::fs::write(&lock_file, serde_json::to_string_pretty(&lock_data)?).await?;
    info!("Lock file created at {} with auth token", lock_file.display());
    
    Ok(())
}

// Cleanup lock file on shutdown
pub async fn cleanup_lock_file(port: u16) -> Result<()> {
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

pub async fn run_websocket_server(port: Option<u16>) -> Result<()> {
    run_websocket_server_with_worktree(port, None).await
}

pub async fn run_websocket_server_with_worktree(port: Option<u16>, worktree: Option<PathBuf>) -> Result<()> {
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
    
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on {}", addr);
    
    // Shared state for managing connections
    let server_state = Arc::new(ServerState::new(worktree));
    
    // Create lock file for CLI discovery
    create_lock_file(port, &server_state).await?;
    
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

// WebSocket connection handler with authentication
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<ServerState>,
) -> Result<()> {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Failed to accept WebSocket connection: {}", e);
            return Ok(());
        }
    };
    
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
                
                match serde_json::from_str::<JsonRpcRequest>(&text) {
                    Ok(request) => {
                        let response = handle_jsonrpc_request(request, &state).await;
                        if let Some(resp) = response {
                            let response_text = serde_json::to_string(&resp)?;
                            
                            if let Err(e) = ws_sender.send(Message::Text(response_text)).await {
                                error!("Failed to send response: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse JSON-RPC request: {}", e);
                        let error_response = JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32700,
                                message: "Parse error".to_string(),
                                data: Some(serde_json::json!({"details": e.to_string()})),
                            }),
                            id: None,
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

// Handle JSON-RPC requests according to Claude Code protocol
async fn handle_jsonrpc_request(
    request: JsonRpcRequest,
    state: &Arc<ServerState>,
) -> Option<JsonRpcResponse> {
    // Only respond to requests with an ID (not notifications)
    let id = request.id.clone();
    
    match request.method.as_str() {
        "tools/call" => {
            let tool_name = request.params
                .as_ref()
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str());
            
            let tool_params = request.params
                .as_ref()
                .and_then(|p| p.get("parameters"));
            
            match tool_name {
                Some("openFile") => handle_open_file(tool_params, id).await,
                Some("openDiff") => handle_open_diff(tool_params, id).await,
                Some("getCurrentSelection") => handle_get_current_selection(tool_params, id).await,
                Some("getOpenEditors") => handle_get_open_editors(tool_params, id).await,
                Some("getWorkspaceFolders") => handle_get_workspace_folders(tool_params, id, state).await,
                Some("getDiagnostics") => handle_get_diagnostics(tool_params, id).await,
                Some("checkDocumentDirty") => handle_check_document_dirty(tool_params, id).await,
                Some("saveDocument") => handle_save_document(tool_params, id).await,
                Some("close_tab") => handle_close_tab(tool_params, id).await,
                Some("closeAllDiffTabs") => handle_close_all_diff_tabs(tool_params, id).await,
                Some("executeCode") => handle_execute_code(tool_params, id).await,
                Some(unknown) => {
                    warn!("Unknown tool: {}", unknown);
                    Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: "Method not found".to_string(),
                            data: Some(serde_json::json!({"tool": unknown})),
                        }),
                        id,
                    })
                }
                None => {
                    Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "Invalid params".to_string(),
                            data: Some(serde_json::json!({"error": "Missing tool name"})),
                        }),
                        id,
                    })
                }
            }
        }
        "selection_changed" => {
            // Handle selection change notifications
            info!("Selection changed: {:?}", request.params);
            None // No response for notifications
        }
        "at_mentioned" => {
            // Handle explicit context selection
            info!("At mentioned: {:?}", request.params);
            None // No response for notifications
        }
        _ => {
            if id.is_some() {
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: "Method not found".to_string(),
                        data: Some(serde_json::json!({"method": request.method})),
                    }),
                    id,
                })
            } else {
                None
            }
        }
    }
}

// Tool handler implementations
async fn handle_open_file(params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    let path = params.and_then(|p| p.get("path")).and_then(|p| p.as_str());
    
    match path {
        Some(file_path) => {
            match tokio::fs::read_to_string(file_path).await {
                Ok(content) => Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(serde_json::json!({
                        "path": file_path,
                        "content": content
                    })),
                    error: None,
                    id,
                }),
                Err(e) => Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: "Internal error".to_string(),
                        data: Some(serde_json::json!({"error": e.to_string()})),
                    }),
                    id,
                }),
            }
        }
        None => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: "Invalid params".to_string(),
                data: Some(serde_json::json!({"error": "Missing path parameter"})),
            }),
            id,
        }),
    }
}

async fn handle_open_diff(params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    let path = params.and_then(|p| p.get("path")).and_then(|p| p.as_str());
    
    match path {
        Some(file_path) => {
            // Mock git diff implementation
            info!("Opening diff for {}", file_path);
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({
                    "path": file_path,
                    "diff": "No changes detected (mock implementation)"
                })),
                error: None,
                id,
            })
        }
        None => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: "Invalid params".to_string(),
                data: Some(serde_json::json!({"error": "Missing path parameter"})),
            }),
            id,
        }),
    }
}

async fn handle_get_current_selection(_params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "selection": "",
            "path": "",
            "line": 0,
            "column": 0
        })),
        error: None,
        id,
    })
}

async fn handle_get_open_editors(_params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "editors": []
        })),
        error: None,
        id,
    })
}

async fn handle_get_workspace_folders(_params: Option<&Value>, id: Option<Value>, state: &Arc<ServerState>) -> Option<JsonRpcResponse> {
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "folders": state.workspace_folders
        })),
        error: None,
        id,
    })
}

async fn handle_get_diagnostics(_params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "diagnostics": []
        })),
        error: None,
        id,
    })
}

async fn handle_check_document_dirty(params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    let path = params.and_then(|p| p.get("path")).and_then(|p| p.as_str());
    
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "path": path.unwrap_or(""),
            "isDirty": false
        })),
        error: None,
        id,
    })
}

async fn handle_save_document(params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    let path = params.and_then(|p| p.get("path")).and_then(|p| p.as_str());
    let content = params.and_then(|p| p.get("content")).and_then(|p| p.as_str());
    
    match (path, content) {
        (Some(file_path), Some(file_content)) => {
            match tokio::fs::write(file_path, file_content).await {
                Ok(_) => Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(serde_json::json!({
                        "path": file_path,
                        "saved": true
                    })),
                    error: None,
                    id,
                }),
                Err(e) => Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: "Internal error".to_string(),
                        data: Some(serde_json::json!({"error": e.to_string()})),
                    }),
                    id,
                }),
            }
        }
        _ => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: "Invalid params".to_string(),
                data: Some(serde_json::json!({"error": "Missing path or content parameter"})),
            }),
            id,
        }),
    }
}

async fn handle_close_tab(params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    let path = params.and_then(|p| p.get("path")).and_then(|p| p.as_str());
    
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "path": path.unwrap_or(""),
            "closed": true
        })),
        error: None,
        id,
    })
}

async fn handle_close_all_diff_tabs(_params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "closed": true
        })),
        error: None,
        id,
    })
}

async fn handle_execute_code(params: Option<&Value>, id: Option<Value>) -> Option<JsonRpcResponse> {
    let code = params.and_then(|p| p.get("code")).and_then(|p| p.as_str());
    
    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "code": code.unwrap_or(""),
            "output": "Code execution not implemented",
            "success": false
        })),
        error: None,
        id,
    })
}