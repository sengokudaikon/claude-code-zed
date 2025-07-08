use anyhow::{anyhow, Result};
use dirs::home_dir;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_hdr_async, tungstenite::Message, WebSocketStream, tungstenite::handshake::server::{Request, Response}};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::mcp::{MCPServer, MCPRequest, MCPResponse};

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    pub pid: u32,
    #[serde(rename = "workspaceFolders")]
    pub workspace_folders: Vec<String>,
    #[serde(rename = "ideName")]
    pub ide_name: String,
    pub transport: String,
    #[serde(rename = "authToken")]
    pub auth_token: String,
}

pub async fn run_websocket_server(port: Option<u16>) -> Result<()> {
    run_websocket_server_with_worktree(port, None).await
}

pub async fn run_websocket_server_with_worktree(
    port: Option<u16>,
    worktree: Option<PathBuf>,
) -> Result<()> {
    info!("Starting WebSocket server...");

    // Generate or use provided port
    let port = port.unwrap_or_else(|| {
        let mut rng = rand::thread_rng();
        rng.gen_range(49152..65535)
    });

    // Create lock file
    let auth_token = Uuid::new_v4().to_string();
    create_lock_file(port, worktree.clone(), &auth_token).await?;

    // Start WebSocket server
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on {}", addr);

    while let Ok((stream, peer_addr)) = listener.accept().await {
        info!("New connection from {}", peer_addr);
        let auth_token_clone = auth_token.clone();
        tokio::spawn(handle_connection(stream, peer_addr, auth_token_clone));
    }

    Ok(())
}

async fn create_lock_file(port: u16, worktree: Option<PathBuf>, auth_token: &str) -> Result<()> {
    let home = home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    let claude_dir = home.join(".claude").join("ide");

    // Create directories if they don't exist
    if !claude_dir.exists() {
        fs::create_dir_all(&claude_dir)?;
        info!("Created directory: {}", claude_dir.display());
    }

    // Get current working directory or use provided worktree
    let workspace_folder = if let Some(wt) = worktree {
        wt.to_string_lossy().to_string()
    } else {
        env::current_dir()?.to_string_lossy().to_string()
    };

    let lock_file_data = LockFile {
        pid: process::id(),
        workspace_folders: vec![workspace_folder],
        ide_name: "claude-code-server".to_string(),
        transport: "ws".to_string(),
        auth_token: auth_token.to_string(),
    };

    let lock_file_path = claude_dir.join(format!("{}.lock", port));
    let json_data = serde_json::to_string_pretty(&lock_file_data)?;

    fs::write(&lock_file_path, json_data)?;
    info!("Created lock file: {}", lock_file_path.display());

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    auth_token: String,
) -> Result<()> {
    info!("Handling connection from {}", peer_addr);

    let ws_stream = match accept_hdr_async(stream, |req: &Request, mut response: Response| {
        // Check if client requested MCP protocol
        if let Some(protocols) = req.headers().get("Sec-WebSocket-Protocol") {
            if let Ok(protocols_str) = protocols.to_str() {
                if protocols_str.contains("mcp") {
                    // Add MCP protocol to response
                    response.headers_mut().insert("Sec-WebSocket-Protocol", "mcp".parse().unwrap());
                    info!("MCP protocol negotiated for {}", peer_addr);
                }
            }
        }
        Ok(response)
    }).await {
        Ok(ws) => {
            info!("WebSocket handshake completed for {}", peer_addr);
            ws
        }
        Err(e) => {
            error!("WebSocket handshake failed for {}: {}", peer_addr, e);
            return Err(e.into());
        }
    };

    handle_websocket_connection(ws_stream, peer_addr, auth_token).await
}

async fn handle_websocket_connection(
    ws_stream: WebSocketStream<TcpStream>,
    peer_addr: SocketAddr,
    _auth_token: String,
) -> Result<()> {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mcp_handler = MCPServer::new();

    info!("WebSocket connection established with {}", peer_addr);

    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(msg) => {
                if msg.is_text() {
                    let text = msg.to_text().unwrap();
                    debug!("Received message from {}: {}", peer_addr, text);

                    // Try to parse as MCP request
                    match serde_json::from_str::<MCPRequest>(text) {
                        Ok(mcp_request) => {
                            info!("Processing MCP request: {}", mcp_request.method);

                            // Handle notifications (requests without ID) separately
                            if mcp_request.id.is_none() && mcp_request.method.starts_with("notifications/") {
                                info!("Processing notification: {}", mcp_request.method);
                                // Notifications don't get responses, just continue
                                continue;
                            }

                            match mcp_handler.handle_request(mcp_request).await {
                                Ok(response) => {
                                    let response_json = serde_json::to_string(&response)?;
                                    debug!("Sending MCP response: {}", response_json);

                                    if let Err(e) =
                                        ws_sender.send(Message::Text(response_json)).await
                                    {
                                        error!(
                                            "Failed to send MCP response to {}: {}",
                                            peer_addr, e
                                        );
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Error handling MCP request: {}", e);
                                    let error_response = MCPResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: None,
                                        result: None,
                                        error: Some(crate::mcp::MCPError {
                                            code: -32603,
                                            message: "Internal error".to_string(),
                                            data: Some(
                                                serde_json::json!({"details": e.to_string()}),
                                            ),
                                        }),
                                    };

                                    let error_json = serde_json::to_string(&error_response)?;
                                    if let Err(e) = ws_sender.send(Message::Text(error_json)).await
                                    {
                                        error!(
                                            "Failed to send error response to {}: {}",
                                            peer_addr, e
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse MCP request from {}: {}", peer_addr, e);
                            debug!("Invalid message content: {}", text);

                            // Send back a JSON-RPC error response
                            let error_response = MCPResponse {
                                jsonrpc: "2.0".to_string(),
                                id: None,
                                result: None,
                                error: Some(crate::mcp::MCPError {
                                    code: -32700,
                                    message: "Parse error".to_string(),
                                    data: None,
                                }),
                            };

                            let error_json = serde_json::to_string(&error_response)?;
                            if let Err(e) = ws_sender.send(Message::Text(error_json)).await {
                                error!(
                                    "Failed to send parse error response to {}: {}",
                                    peer_addr, e
                                );
                                break;
                            }
                        }
                    }
                } else if msg.is_close() {
                    info!("Connection closed by {}", peer_addr);
                    break;
                }
            }
            Err(e) => {
                error!("WebSocket error for {}: {}", peer_addr, e);
                break;
            }
        }
    }

    info!("WebSocket connection with {} ended", peer_addr);
    Ok(())
}
