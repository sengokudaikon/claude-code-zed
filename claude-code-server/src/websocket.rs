use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::interval;
use tokio_tungstenite::{tungstenite::Message};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::tools::{ToolRegistry, create_default_registry};

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

// MCP protocol constants
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

// MCP error codes
const PARSE_ERROR: i32 = -32700;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;

// MCP capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCapabilities {
    pub logging: serde_json::Map<String, Value>,
    pub prompts: McpPromptsCapability,
    pub resources: McpResourcesCapability,
    pub tools: McpToolsCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourcesCapability {
    pub subscribe: bool,
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug)]
pub struct ConnectionInfo {
    pub addr: String,
    pub last_ping: Instant,
    pub last_pong: Instant,
}

#[derive(Debug)]
pub struct ServerState {
    pub connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    pub auth_token: String,
    pub workspace_folders: Vec<String>,
    pub ide_name: String,
    pub tool_registry: ToolRegistry,
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
            tool_registry: create_default_registry(),
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
    
    debug!("Lock file directory: {}", lock_dir.display());
    
    tokio::fs::create_dir_all(&lock_dir).await.map_err(|e| {
        error!("Failed to create lock directory {}: {}", lock_dir.display(), e);
        debug!("Directory creation error details: {:?}", e);
        e
    })?;
    
    debug!("Lock directory created/verified: {}", lock_dir.display());
    
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
    
    let lock_json = serde_json::to_string_pretty(&lock_data).map_err(|e| {
        error!("Failed to serialize lock file data: {}", e);
        debug!("Serialization error details: {:?}", e);
        e
    })?;
    
    debug!("Writing lock file content: {}", lock_json);
    
    tokio::fs::write(&lock_file, &lock_json).await.map_err(|e| {
        error!("Failed to write lock file {}: {}", lock_file.display(), e);
        debug!("File write error details: {:?}", e);
        e
    })?;
    
    info!("Lock file created at {} with auth token (length: {})", 
          lock_file.display(), state.auth_token.len());
    debug!("Lock file content written successfully: {} bytes", lock_json.len());
    
    Ok(())
}

// Cleanup lock file on shutdown
pub async fn cleanup_lock_file(port: u16) -> Result<()> {
    let lock_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude")
        .join("ide");
    
    let lock_file = lock_dir.join(format!("{}.lock", port));
    
    debug!("Attempting to cleanup lock file: {}", lock_file.display());
    
    if lock_file.exists() {
        debug!("Lock file exists, removing: {}", lock_file.display());
        tokio::fs::remove_file(&lock_file).await.map_err(|e| {
            error!("Failed to remove lock file {}: {}", lock_file.display(), e);
            debug!("Lock file removal error details: {:?}", e);
            e
        })?;
        info!("Lock file cleaned up: {}", lock_file.display());
    } else {
        debug!("Lock file does not exist, no cleanup needed: {}", lock_file.display());
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
    debug!("Attempting to bind TCP listener to {}", addr);
    
    let listener = TcpListener::bind(&addr).await.map_err(|e| {
        error!("Failed to bind WebSocket server to {}: {}", addr, e);
        debug!("Bind error details: {:?}", e);
        e
    })?;
    
    info!("WebSocket server successfully listening on {}", addr);
    debug!("TCP listener bound and ready to accept connections");
    
    // Shared state for managing connections
    let server_state = Arc::new(ServerState::new(worktree));
    
    // Create lock file for CLI discovery
    debug!("Creating lock file for port {}", port);
    create_lock_file(port, &server_state).await.map_err(|e| {
        error!("Failed to create lock file for port {}: {}", port, e);
        debug!("Lock file creation error details: {:?}", e);
        e
    })?;
    debug!("Lock file created successfully");
    
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
    
    // Start ping keepalive task
    let ping_state = Arc::clone(&server_state);
    tokio::spawn(ping_keepalive_task(ping_state));
    
    while let Ok((stream, addr)) = listener.accept().await {
        info!("New TCP connection from {}", addr);
        
        // Log detailed connection information
        if let Ok(peer_addr) = stream.peer_addr() {
            debug!("Peer address confirmed: {}", peer_addr);
        }
        if let Ok(local_addr) = stream.local_addr() {
            debug!("Local address: {}", local_addr);
        }
        
        // Log socket options for debugging
        debug!("TCP connection details for {}: nodelay={:?}, keepalive={:?}", 
               addr, 
               stream.nodelay().unwrap_or(false),
               "unknown" // keepalive info not easily accessible
        );
        
        let state = Arc::clone(&server_state);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, state).await {
                error!("Connection handler error for {}: {}", addr, e);
                debug!("Connection handler error details: {:?}", e);
            }
        });
    }
    
    Ok(())
}

// WebSocket connection handler with authentication
async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    state: Arc<ServerState>,
) -> Result<()> {
    debug!("Starting WebSocket handshake for connection from {}", addr);
    
    // Log TCP connection details
    if let Ok(local_addr) = stream.local_addr() {
        debug!("Local endpoint: {}", local_addr);
    }
    if let Ok(peer_addr) = stream.peer_addr() {
        debug!("Peer endpoint: {}", peer_addr);
    }
    
    // Capture initial handshake attempt with detailed error context
    let ws_stream = match accept_async_with_context(&mut stream, addr).await {
        Ok(ws) => {
            info!("WebSocket handshake successful for {}", addr);
            ws
        },
        Err(e) => {
            error!("Failed to accept WebSocket connection from {}: {}", addr, e);
            debug!("WebSocket handshake error details: {:?}", e);
            return Ok(());
        }
    };
    
    let connection_id = Uuid::new_v4().to_string();
    info!("WebSocket connection established: {} ({})", connection_id, addr);
    debug!("WebSocket connection details - ID: {}, Address: {}", connection_id, addr);
    
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    debug!("WebSocket stream split successfully for {}", connection_id);
    
    // Store connection ID with connection info
    {
        let mut connections = state.connections.write().await;
        let now = Instant::now();
        connections.insert(connection_id.clone(), ConnectionInfo {
            addr: addr.to_string(),
            last_ping: now,
            last_pong: now,
        });
    }
    
    // Handle incoming messages
    debug!("Starting message loop for connection: {}", connection_id);
    while let Some(msg) = ws_receiver.next().await {
        debug!("Received WebSocket message from {}: {:?}", connection_id, msg);
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Processing text message from {}: {}", connection_id, text);
                
                match serde_json::from_str::<JsonRpcRequest>(&text) {
                    Ok(request) => {
                        debug!("Parsed JSON-RPC request from {}: method={}, id={:?}", connection_id, request.method, request.id);
                        let response = handle_jsonrpc_request(request, &state).await;
                        if let Some(resp) = response {
                            let response_text = serde_json::to_string(&resp)?;
                            debug!("Sending response to {}: {}", connection_id, response_text);
                            
                            if let Err(e) = ws_sender.send(Message::Text(response_text)).await {
                                error!("Failed to send response to {}: {}", connection_id, e);
                                debug!("WebSocket send error details: {:?}", e);
                                break;
                            }
                        } else {
                            debug!("No response needed for request from {}", connection_id);
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse JSON-RPC request from {}: {}", connection_id, e);
                        debug!("Invalid JSON received from {}: {}", connection_id, text);
                        debug!("Parse error details: {:?}", e);
                        
                        let error_response = JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(JsonRpcError {
                                code: PARSE_ERROR,
                                message: "Parse error".to_string(),
                                data: Some(serde_json::json!({
                                    "details": e.to_string(),
                                    "received_text": text.chars().take(200).collect::<String>() // First 200 chars for debugging
                                })),
                            }),
                            id: None,
                        };
                        
                        if let Ok(response_text) = serde_json::to_string(&error_response) {
                            debug!("Sending parse error response to {}", connection_id);
                            let _ = ws_sender.send(Message::Text(response_text)).await;
                        } else {
                            error!("Failed to serialize error response for {}", connection_id);
                        }
                    }
                }
            }
            Ok(Message::Close(close_frame)) => {
                info!("WebSocket connection closed by client: {}", connection_id);
                debug!("Close frame details: {:?}", close_frame);
                break;
            }
            Ok(Message::Ping(payload)) => {
                if let Err(e) = ws_sender.send(Message::Pong(payload)).await {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {
                // Update last pong time for keepalive tracking
                {
                    let mut connections = state.connections.write().await;
                    if let Some(conn_info) = connections.get_mut(&connection_id) {
                        let now = Instant::now();
                        conn_info.last_pong = now;
                        debug!("Received pong from connection: {} at {:?}", connection_id, now);
                    } else {
                        warn!("Received pong from unknown connection: {}", connection_id);
                    }
                }
            }
            Ok(Message::Binary(data)) => {
                warn!("Received binary message from {}, ignoring (length: {})", connection_id, data.len());
                debug!("Binary data preview: {:?}", data.get(0..std::cmp::min(20, data.len())));
            }
            Ok(Message::Frame(frame)) => {
                // Handle frame messages (typically handled internally)
                debug!("Received frame message from {}: {:?}", connection_id, frame);
            }
            Err(e) => {
                error!("WebSocket error on connection {}: {}", connection_id, e);
                debug!("WebSocket error details: {:?}", e);
                
                // Try to categorize the error for better debugging
                if e.to_string().contains("Connection reset") {
                    info!("Client {} disconnected abruptly (connection reset)", connection_id);
                } else if e.to_string().contains("Protocol") {
                    warn!("WebSocket protocol error on {}: possibly invalid client", connection_id);
                } else if e.to_string().contains("Closed") {
                    info!("WebSocket connection {} closed normally", connection_id);
                } else {
                    warn!("Unexpected WebSocket error on {}: {}", connection_id, e);
                }
                break;
            }
        }
    }
    
    // Clean up connection
    {
        let mut connections = state.connections.write().await;
        if connections.remove(&connection_id).is_some() {
            debug!("Connection {} removed from active connections list", connection_id);
        } else {
            warn!("Connection {} was not found in active connections list during cleanup", connection_id);
        }
    }
    
    info!("WebSocket connection handler finished: {} ({})", connection_id, addr);
    debug!("Final cleanup completed for connection {}", connection_id);
    Ok(())
}

// Handle JSON-RPC requests according to Claude Code protocol
async fn handle_jsonrpc_request(
    request: JsonRpcRequest,
    state: &Arc<ServerState>,
) -> Option<JsonRpcResponse> {
    // Only respond to requests with an ID (not notifications)
    let id = request.id.clone();
    
    debug!("Handling JSON-RPC request: method={}, id={:?}, has_params={}", 
           request.method, id, request.params.is_some());
    
    if let Some(ref params) = request.params {
        debug!("Request parameters: {}", serde_json::to_string(params).unwrap_or_else(|_| "<invalid_json>".to_string()));
    }
    
    match request.method.as_str() {
        "initialize" => {
            info!("Received initialize request");
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": McpCapabilities {
                        logging: serde_json::Map::new(),
                        prompts: McpPromptsCapability { list_changed: true },
                        resources: McpResourcesCapability { subscribe: true, list_changed: true },
                        tools: McpToolsCapability { list_changed: true },
                    },
                    "serverInfo": McpServerInfo {
                        name: "claude-code-server".to_string(),
                        version: "0.1.0".to_string(),
                    }
                })),
                error: None,
                id,
            })
        }
        "prompts/list" => {
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({
                    "prompts": []
                })),
                error: None,
                id,
            })
        }
        "tools/list" => {
            let tool_list = state.tool_registry.get_tool_list();
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({
                    "tools": tool_list
                })),
                error: None,
                id,
            })
        }
        "tools/call" => {
            let tool_name = request.params
                .as_ref()
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str());
            
            let default_params = serde_json::json!({});
            let tool_params = request.params
                .as_ref()
                .and_then(|p| p.get("arguments"))
                .unwrap_or(&default_params);
            
            match tool_name {
                Some(name) => {
                    debug!("Calling tool: {} with params: {:?}", name, tool_params);
                    match state.tool_registry.call_tool(name, tool_params) {
                        Ok(result) => {
                            debug!("Tool {} completed successfully", name);
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                result: Some(result),
                                error: None,
                                id,
                            })
                        }
                        Err(tool_error) => {
                            warn!("Tool {} failed: {:?}", name, tool_error);
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                result: None,
                                error: Some(JsonRpcError {
                                    code: tool_error.code,
                                    message: tool_error.message,
                                    data: tool_error.data,
                                }),
                                id,
                            })
                        }
                    }
                }
                None => {
                    Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: INVALID_PARAMS,
                            message: "Invalid params".to_string(),
                            data: Some(serde_json::json!({"error": "Missing tool name"})),
                        }),
                        id,
                    })
                }
            }
        }
        "notifications/initialized" => {
            info!("Client initialized");
            None // No response for notifications
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
                        code: INTERNAL_ERROR,
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
                code: INVALID_PARAMS,
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
                code: INVALID_PARAMS,
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
                        code: INTERNAL_ERROR,
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
                code: INVALID_PARAMS,
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

// Get the list of available tools with their schemas
fn get_tool_list() -> Vec<Value> {
    vec![
        serde_json::json!({
            "name": "openFile",
            "description": "Opens a file in the editor",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to open"
                    }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "getCurrentSelection",
            "description": "Gets the current text selection in the editor",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "getOpenEditors",
            "description": "Gets a list of currently open editors",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "openDiff",
            "description": "Opens a diff view for a file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to diff"
                    }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "saveDocument",
            "description": "Saves a document with the given content",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to save"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to save"
                    }
                },
                "required": ["path", "content"]
            }
        }),
        serde_json::json!({
            "name": "getWorkspaceFolders",
            "description": "Gets the current workspace folders",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "getDiagnostics",
            "description": "Gets diagnostics for the current workspace",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "checkDocumentDirty",
            "description": "Checks if a document has unsaved changes",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the document to check"
                    }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "close_tab",
            "description": "Closes a tab in the editor",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to close"
                    }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "closeAllDiffTabs",
            "description": "Closes all diff tabs in the editor",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "executeCode",
            "description": "Executes code in the terminal",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Code to execute"
                    }
                },
                "required": ["code"]
            }
        }),
    ]
}

// Ping keepalive task to maintain WebSocket connections
async fn ping_keepalive_task(state: Arc<ServerState>) {
    let mut interval = interval(Duration::from_secs(30)); // Ping every 30 seconds
    let timeout_duration = Duration::from_secs(60); // Consider connection dead after 60 seconds
    
    info!("Starting ping keepalive task (ping every 30s, timeout after 60s)");
    
    loop {
        interval.tick().await;
        
        let mut connections_to_remove = Vec::new();
        let now = Instant::now();
        
        debug!("Running connection keepalive check at {:?}", now);
        
        // Check all connections for timeout
        {
            let connections = state.connections.read().await;
            debug!("Checking {} active connections for timeout", connections.len());
            
            for (connection_id, conn_info) in connections.iter() {
                let time_since_pong = now.duration_since(conn_info.last_pong);
                
                debug!("Connection {}: last_pong was {:.1}s ago (timeout at {:.1}s)", 
                       connection_id, 
                       time_since_pong.as_secs_f64(),
                       timeout_duration.as_secs_f64());
                
                // Check if connection is still alive (received pong within timeout)
                if time_since_pong > timeout_duration {
                    warn!("Connection {} appears dead (last pong {:.1}s ago), marking for removal", 
                          connection_id, time_since_pong.as_secs_f64());
                    connections_to_remove.push(connection_id.clone());
                } else {
                    debug!("Connection {} is healthy (last pong {:.1}s ago)", 
                           connection_id, time_since_pong.as_secs_f64());
                }
            }
        }
        
        // Remove dead connections
        if !connections_to_remove.is_empty() {
            warn!("Removing {} dead connections", connections_to_remove.len());
            let mut connections = state.connections.write().await;
            for connection_id in &connections_to_remove {
                if connections.remove(connection_id).is_some() {
                    info!("Removed dead connection: {}", connection_id);
                } else {
                    warn!("Connection {} was already removed", connection_id);
                }
            }
        }
        
        // Log connection count
        let connection_count = state.connections.read().await.len();
        if connection_count > 0 {
            debug!("Keepalive check complete: {} active connections remaining", connection_count);
        } else {
            debug!("No active connections");
        }
    }
}

// Enhanced WebSocket accept with detailed context logging
async fn accept_async_with_context(
    stream: &mut TcpStream,
    addr: SocketAddr,
) -> Result<tokio_tungstenite::WebSocketStream<&mut TcpStream>, tokio_tungstenite::tungstenite::Error> {
    debug!("Starting enhanced WebSocket handshake analysis for {}", addr);
    
    // Try to peek at the initial data to analyze the request
    let mut peek_buffer = [0u8; 1024];
    match stream.try_read(&mut peek_buffer) {
        Ok(n) if n > 0 => {
            debug!("Read {} bytes from TCP stream for analysis", n);
            let request_data = String::from_utf8_lossy(&peek_buffer[..n]);
            debug!("Raw HTTP request from {}:\n{}", addr, request_data);
            
            // Analyze HTTP headers
            analyze_http_request(&request_data, addr);
        }
        Ok(_) => {
            warn!("No data available to read from {}", addr);
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            debug!("No immediate data available from {} (would block)", addr);
        }
        Err(e) => {
            warn!("Error peeking at TCP data from {}: {}", addr, e);
        }
    }
    
    // Proceed with normal WebSocket handshake
    tokio_tungstenite::accept_async(stream).await
}

// Analyze HTTP request headers for debugging
fn analyze_http_request(request_data: &str, addr: SocketAddr) {
    debug!("Analyzing HTTP request from {}", addr);
    
    let lines: Vec<&str> = request_data.lines().collect();
    if lines.is_empty() {
        warn!("Empty HTTP request from {}", addr);
        return;
    }
    
    // Parse request line
    let request_line = lines[0];
    debug!("Request line from {}: {}", addr, request_line);
    
    if !request_line.starts_with("GET") {
        warn!("Non-GET request from {}: {}", addr, request_line);
    }
    
    // Parse headers
    let mut headers = HashMap::new();
    for line in lines.iter().skip(1) {
        if line.is_empty() {
            break; // End of headers
        }
        
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim();
            headers.insert(key.clone(), value.to_string());
            debug!("Header from {}: {} = {}", addr, key, value);
        }
    }
    
    // Check required WebSocket headers
    debug!("Checking WebSocket headers for {}", addr);
    
    // Check Connection header
    match headers.get("connection") {
        Some(conn) if conn.to_lowercase().contains("upgrade") => {
            debug!("✓ Connection header OK for {}: {}", addr, conn);
        }
        Some(conn) => {
            error!("✗ Invalid Connection header from {}: '{}' (should contain 'upgrade')", addr, conn);
        }
        None => {
            error!("✗ Missing Connection header from {}", addr);
        }
    }
    
    // Check Upgrade header
    match headers.get("upgrade") {
        Some(upgrade) if upgrade.to_lowercase() == "websocket" => {
            debug!("✓ Upgrade header OK for {}: {}", addr, upgrade);
        }
        Some(upgrade) => {
            error!("✗ Invalid Upgrade header from {}: '{}' (should be 'websocket')", addr, upgrade);
        }
        None => {
            error!("✗ Missing Upgrade header from {}", addr);
        }
    }
    
    // Check WebSocket version
    match headers.get("sec-websocket-version") {
        Some(version) if version == "13" => {
            debug!("✓ WebSocket version OK for {}: {}", addr, version);
        }
        Some(version) => {
            warn!("⚠ Unusual WebSocket version from {}: {} (expected 13)", addr, version);
        }
        None => {
            error!("✗ Missing Sec-WebSocket-Version header from {}", addr);
        }
    }
    
    // Check WebSocket key
    match headers.get("sec-websocket-key") {
        Some(key) => {
            debug!("✓ WebSocket key present for {}: {} chars", addr, key.len());
        }
        None => {
            error!("✗ Missing Sec-WebSocket-Key header from {}", addr);
        }
    }
    
    // Check Host header
    match headers.get("host") {
        Some(host) => {
            debug!("✓ Host header for {}: {}", addr, host);
        }
        None => {
            warn!("⚠ Missing Host header from {}", addr);
        }
    }
    
    // Check User-Agent
    match headers.get("user-agent") {
        Some(ua) => {
            debug!("User-Agent from {}: {}", addr, ua);
        }
        None => {
            debug!("No User-Agent header from {}", addr);
        }
    }
    
    // Check Origin
    match headers.get("origin") {
        Some(origin) => {
            debug!("Origin from {}: {}", addr, origin);
        }
        None => {
            debug!("No Origin header from {}", addr);
        }
    }
    
    // Log all headers for complete debugging
    debug!("All headers from {} (count: {}):", addr, headers.len());
    for (key, value) in &headers {
        debug!("  {}: {}", key, value);
    }
    
    // Determine likely issue
    if !headers.contains_key("connection") || !headers.contains_key("upgrade") {
        error!("❌ DIAGNOSIS for {}: Missing required WebSocket headers - client may not be sending proper WebSocket upgrade request", addr);
    } else if !headers.get("connection").unwrap().to_lowercase().contains("upgrade") {
        error!("❌ DIAGNOSIS for {}: Connection header does not contain 'upgrade' - this is the exact error being reported", addr);
    } else if headers.get("upgrade").unwrap().to_lowercase() != "websocket" {
        error!("❌ DIAGNOSIS for {}: Upgrade header is not 'websocket'", addr);
    } else {
        debug!("✅ DIAGNOSIS for {}: WebSocket headers appear correct - error may be elsewhere", addr);
    }
}