use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use uuid::Uuid;
use zed_extension_api::*;

struct ClaudeCodeExtension {
    server: Arc<Mutex<Option<ClaudeCodeServer>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockFileData {
    pid: u32,
    #[serde(rename = "workspaceFolders")]
    workspace_folders: Vec<String>,
    #[serde(rename = "ideName")]
    ide_name: String,
    transport: String,
    #[serde(rename = "authToken")]
    auth_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcMessage {
    jsonrpc: String,
    method: Option<String>,
    params: Option<Value>,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelectionData {
    text: String,
    #[serde(rename = "filePath")]
    file_path: String,
    #[serde(rename = "fileUrl")]
    file_url: String,
    selection: SelectionRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelectionRange {
    start: Position,
    end: Position,
    #[serde(rename = "isEmpty")]
    is_empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Position {
    line: u32,
    character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AtMentionParams {
    #[serde(rename = "filePath")]
    file_path: String,
    #[serde(rename = "lineStart")]
    line_start: Option<u32>,
    #[serde(rename = "lineEnd")]
    line_end: Option<u32>,
}

struct ClaudeCodeServer {
    port: u16,
    auth_token: String,
    workspace_folders: Vec<String>,
    sender: broadcast::Sender<JsonRpcMessage>,
}

impl ClaudeCodeServer {
    fn new(workspace_folders: Vec<String>) -> Result<Self, String> {
        let auth_token = Uuid::new_v4().to_string();
        let (sender, _) = broadcast::channel(1000);
        
        Ok(Self {
            port: 0, // Will be set when server starts
            auth_token,
            workspace_folders,
            sender,
        })
    }

    async fn start(&mut self) -> Result<u16, String> {
        let listener = TcpListener::bind("127.0.0.1:0").await
            .map_err(|e| format!("Failed to bind to port: {}", e))?;
            
        let addr = listener.local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?;
            
        self.port = addr.port();
        
        // Create lock file
        self.create_lock_file()?;
        
        // Spawn server task
        let auth_token = self.auth_token.clone();
        let sender = self.sender.clone();
        
        tokio::spawn(async move {
            Self::run_server(listener, auth_token, sender).await;
        });
        
        Ok(self.port)
    }

    fn create_lock_file(&self) -> Result<(), String> {
        let home_dir = dirs::home_dir()
            .ok_or("Could not find home directory")?;
            
        let claude_dir = home_dir.join(".claude").join("ide");
        fs::create_dir_all(&claude_dir)
            .map_err(|e| format!("Failed to create .claude/ide directory: {}", e))?;
            
        let lock_file_path = claude_dir.join(format!("{}.lock", self.port));
        
        let lock_data = LockFileData {
            pid: std::process::id(),
            workspace_folders: self.workspace_folders.clone(),
            ide_name: "Zed".to_string(),
            transport: "ws".to_string(),
            auth_token: self.auth_token.clone(),
        };
        
        let lock_content = serde_json::to_string_pretty(&lock_data)
            .map_err(|e| format!("Failed to serialize lock file: {}", e))?;
            
        fs::write(&lock_file_path, lock_content)
            .map_err(|e| format!("Failed to write lock file: {}", e))?;
            
        Ok(())
    }

    async fn run_server(
        listener: TcpListener,
        auth_token: String,
        sender: broadcast::Sender<JsonRpcMessage>,
    ) {
        while let Ok((stream, _)) = listener.accept().await {
            let auth_token = auth_token.clone();
            let sender = sender.clone();
            
            tokio::spawn(async move {
                let callback = |req: &Request, response: Response| {
                    if let Some(auth_header) = req.headers().get("x-claude-code-ide-authorization") {
                        if let Ok(auth_value) = auth_header.to_str() {
                            if auth_value == auth_token {
                                return Ok(response);
                            }
                        }
                    }
                    
                    Err(http::Response::builder()
                        .status(401)
                        .body(Some("Unauthorized".to_string()))
                        .unwrap())
                };
                
                if let Ok(ws_stream) = accept_hdr_async(stream, callback).await {
                    Self::handle_client(ws_stream, sender).await;
                }
            });
        }
    }

    async fn handle_client(
        ws_stream: WebSocketStream<tokio::net::TcpStream>,
        sender: broadcast::Sender<JsonRpcMessage>,
    ) {
        let mut receiver = sender.subscribe();
        let (ws_sender, mut ws_receiver) = ws_stream.split();
        let ws_sender = Arc::new(Mutex::new(ws_sender));
        
        // Handle incoming messages from Claude
        let ws_sender_clone = ws_sender.clone();
        tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                if let Ok(Message::Text(text)) = msg {
                    if let Ok(json_msg) = serde_json::from_str::<JsonRpcMessage>(&text) {
                        Self::handle_claude_message(json_msg, &ws_sender_clone).await;
                    }
                }
            }
        });
        
        // Handle outgoing messages to Claude
        while let Ok(msg) = receiver.recv().await {
            if let Ok(json_text) = serde_json::to_string(&msg) {
                if let Ok(mut sender_guard) = ws_sender.lock() {
                    if sender_guard.send(Message::Text(json_text)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    async fn handle_claude_message(
        msg: JsonRpcMessage,
        ws_sender: &Arc<Mutex<futures_util::stream::SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>>>,
    ) {
        if let Some(method) = &msg.method {
            match method.as_str() {
                "tools/call" => {
                    if let Some(params) = &msg.params {
                        Self::handle_tool_call(msg.id.clone(), params, ws_sender).await;
                    }
                }
                _ => {
                    // Handle other methods if needed
                }
            }
        }
    }

    async fn handle_tool_call(
        id: Option<Value>,
        params: &Value,
        ws_sender: &Arc<Mutex<futures_util::stream::SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>>>,
    ) {
        if let Some(tool_name) = params.get("name").and_then(|n| n.as_str()) {
            let result = match tool_name {
                "openFile" => Self::handle_open_file(params).await,
                "getCurrentSelection" => Self::handle_get_current_selection().await,
                "getWorkspaceFolders" => Self::handle_get_workspace_folders().await,
                "getOpenEditors" => Self::handle_get_open_editors().await,
                _ => {
                    serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Tool '{}' not implemented", tool_name)
                        }]
                    })
                }
            };
            
            let response = JsonRpcMessage {
                jsonrpc: "2.0".to_string(),
                method: None,
                params: None,
                id,
                result: Some(result),
                error: None,
            };
            
            if let Ok(response_text) = serde_json::to_string(&response) {
                if let Ok(mut sender_guard) = ws_sender.lock() {
                    let _ = sender_guard.send(Message::Text(response_text)).await;
                }
            }
        }
    }

    async fn handle_open_file(params: &Value) -> Value {
        if let Some(args) = params.get("arguments") {
            if let Some(file_path) = args.get("filePath").and_then(|f| f.as_str()) {
                // Use Zed's API to open file
                // This is a placeholder - actual implementation would use Zed's extension API
                return serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Opened file: {}", file_path)
                    }]
                });
            }
        }
        
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": "Failed to open file - invalid parameters"
            }]
        })
    }

    async fn handle_get_current_selection() -> Value {
        // This would interface with Zed's selection API
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": r#"{"success": false, "message": "No active selection"}"#
            }]
        })
    }

    async fn handle_get_workspace_folders() -> Value {
        // This would get workspace folders from Zed
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": r#"{"success": true, "folders": [], "rootPath": ""}"#
            }]
        })
    }

    async fn handle_get_open_editors() -> Value {
        // This would get open editors from Zed
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": r#"{"tabs": []}"#
            }]
        })
    }

    fn broadcast_selection_changed(&self, selection: SelectionData) {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            method: Some("selection_changed".to_string()),
            params: Some(serde_json::to_value(selection).unwrap_or_default()),
            id: None,
            result: None,
            error: None,
        };
        
        let _ = self.sender.send(msg);
    }

    fn broadcast_at_mention(&self, params: AtMentionParams) {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            method: Some("at_mentioned".to_string()),
            params: Some(serde_json::to_value(params).unwrap_or_default()),
            id: None,
            result: None,
            error: None,
        };
        
        let _ = self.sender.send(msg);
    }
}

impl Extension for ClaudeCodeExtension {
    fn new() -> Self {
        Self {
            server: Arc::new(Mutex::new(None)),
        }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> Result<Command, String> {
        // This extension doesn't provide language servers
        Err("Claude Code extension does not provide language servers".to_string())
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>, String> {
        Ok(None)
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>, String> {
        Ok(None)
    }

    fn label_for_completion(
        &self,
        _language_server_id: &LanguageServerId,
        _completion: Completion,
        _worktree: &Worktree,
    ) -> Option<CodeLabel> {
        None
    }

    fn label_for_symbol(
        &self,
        _language_server_id: &LanguageServerId,
        _symbol: Symbol,
        _worktree: &Worktree,
    ) -> Option<CodeLabel> {
        None
    }

    fn complete_slash_command_argument(
        &self,
        _command: SlashCommand,
        _args: Vec<String>,
    ) -> Result<Vec<SlashCommandArgumentCompletion>, String> {
        Ok(vec![])
    }

    fn run_slash_command(
        &self,
        _command: SlashCommand,
        _args: Vec<String>,
        _worktree: &Worktree,
    ) -> Result<SlashCommandOutput, String> {
        Ok(SlashCommandOutput {
            text: "Claude Code slash command not implemented".to_string(),
            sections: vec![],
        })
    }
}

impl ClaudeCodeExtension {
    fn start_server(&self) -> Result<u16, String> {
        let workspace_folders = self.get_workspace_folders();
        let mut server = ClaudeCodeServer::new(workspace_folders)?;
        
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
            
        let port = runtime.block_on(async {
            server.start().await
        })?;
        
        // Store the server instance
        if let Ok(mut server_guard) = self.server.lock() {
            *server_guard = Some(server);
        }
        
        // Set environment variable for Claude Code to find the server
        std::env::set_var("CLAUDE_CODE_SSE_PORT", port.to_string());
        std::env::set_var("ENABLE_IDE_INTEGRATION", "true");
        
        Ok(port)
    }

    fn get_workspace_folders(&self) -> Vec<String> {
        // This would use Zed's API to get workspace folders
        // For now, return current directory
        vec![std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()]
    }

    fn send_at_mention(&self, file_path: String, line_start: Option<u32>, line_end: Option<u32>) {
        if let Ok(server_guard) = self.server.lock() {
            if let Some(server) = server_guard.as_ref() {
                let params = AtMentionParams {
                    file_path,
                    line_start,
                    line_end,
                };
                server.broadcast_at_mention(params);
            }
        }
    }
}

zed_extension_api::register_extension!(ClaudeCodeExtension);