use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use zed_extension_api::{lsp::*, *};
use rand::Rng;

// Simple logging macros with levels
macro_rules! log_debug {
    ($($arg:tt)*) => {
        eprintln!("🔍 [DEBUG] Claude Code: {}", format!($($arg)*));
    };
}

macro_rules! log_info {
    ($($arg:tt)*) => {
        eprintln!("ℹ️ [INFO] Claude Code: {}", format!($($arg)*));
    };
}

macro_rules! log_warn {
    ($($arg:tt)*) => {
        eprintln!("⚠️ [WARN] Claude Code: {}", format!($($arg)*));
    };
}

macro_rules! log_error {
    ($($arg:tt)*) => {
        eprintln!("❌ [ERROR] Claude Code: {}", format!($($arg)*));
    };
}

macro_rules! log_success {
    ($($arg:tt)*) => {
        eprintln!("✅ [SUCCESS] Claude Code: {}", format!($($arg)*));
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcMessage {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
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
    line_start: u32,
    #[serde(rename = "lineEnd")]
    line_end: u32,
}

struct ClaudeCodeServer {
    port: u16,
    auth_token: String,
    workspace_folders: Vec<String>,
}

struct ClaudeCodeExtension {
    server_config: Option<ClaudeCodeServer>,
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

impl Extension for ClaudeCodeExtension {
    fn new() -> Self {
        let mut extension = Self { server_config: None };

        // Initialize the server configuration
        if let Ok(server) = extension.init_server_config() {
            extension.server_config = Some(server);
            log_success!("Claude Code server configuration initialized successfully");
        } else {
            log_error!("Failed to initialize Claude Code server configuration");
        }

        extension
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> Result<Command, String> {
        log_debug!(
            "language_server_command called for {:?}",
            language_server_id
        );
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
    ) -> Option<CodeLabel> {
        None
    }

    fn label_for_symbol(
        &self,
        _language_server_id: &LanguageServerId,
        _symbol: Symbol,
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
        command: SlashCommand,
        args: Vec<String>,
        _worktree: Option<&Worktree>,
    ) -> Result<SlashCommandOutput, String> {
        log_debug!(
            "Slash command '{}' called with args: {:?}",
            command.name,
            args
        );
        Ok(SlashCommandOutput {
            text: format!(
                "Claude Code slash command '{}' not yet implemented",
                command.name
            ),
            sections: vec![],
        })
    }
}

impl ClaudeCodeExtension {
    /// Initialize the server configuration (WASM-compatible)
    fn init_server_config(&self) -> Result<ClaudeCodeServer, Box<dyn std::error::Error>> {
        log_debug!("Initializing Claude Code server configuration...");
        
        // Generate random port in range 10000-65535
        let port = self.generate_random_port();
        let auth_token = Uuid::new_v4().to_string();
        let workspace_folders = self.get_workspace_folders();
        
        let server = ClaudeCodeServer {
            port,
            auth_token: auth_token.clone(),
            workspace_folders: workspace_folders.clone(),
        };
        
        // Prepare lock file data (actual creation would require Zed API)
        let _lock_data = self.create_lock_file_data(&server);
        
        // Log environment variables that would be set
        self.log_environment_variables(port);
        
        log_info!("Server config prepared for port {} with auth token {}...", port, &auth_token[..8]);
        Ok(server)
    }
    
    /// Generate random port in range 10000-65535
    fn generate_random_port(&self) -> u16 {
        let mut rng = rand::thread_rng();
        rng.gen_range(10000..=65535)
    }
    
    /// Create lock file data structure
    fn create_lock_file_data(&self, server: &ClaudeCodeServer) -> LockFileData {
        log_debug!("Creating lock file data for port {}", server.port);
        
        let lock_data = LockFileData {
            pid: 12345, // Placeholder PID for WASM
            workspace_folders: server.workspace_folders.clone(),
            ide_name: "Zed".to_string(),
            transport: "ws".to_string(),
            auth_token: server.auth_token.clone(),
        };
        
        log_info!("Lock file data prepared: {:?}", lock_data);
        lock_data
    }
    
    /// Log environment variables for Claude Code discovery
    fn log_environment_variables(&self, port: u16) {
        log_debug!("Environment variables for port {}", port);
        log_info!("CLAUDE_CODE_SSE_PORT={}, ENABLE_IDE_INTEGRATION=true", port);
    }

    /// Get workspace folders (WASM-compatible implementation)
    fn get_workspace_folders(&self) -> Vec<String> {
        log_debug!("Getting workspace folders...");
        // In WASM, we can't access filesystem directly
        // This would need to use Zed's API to get workspace information
        let folders = vec!["/workspace".to_string()]; // Placeholder for MVP
        log_info!("Found {} workspace folder(s): {:?}", folders.len(), folders);
        folders
    }

    /// Handle incoming WebSocket messages
    fn handle_websocket_message(&self, message: &str, _auth_token: &str) -> Option<String> {
        log_debug!("Handling WebSocket message: {}", message);
        
        // Parse JSON-RPC message
        let rpc_message: JsonRpcMessage = match serde_json::from_str(message) {
            Ok(msg) => msg,
            Err(e) => {
                log_error!("Failed to parse JSON-RPC message: {}", e);
                return Some(self.create_error_response(None, -32700, "Parse error"));
            }
        };
        
        // Handle method calls (MCP tools)
        if let Some(method) = &rpc_message.method {
            let result = self.handle_tool_call(method, &rpc_message.params.unwrap_or(Value::Null));
            return Some(self.create_success_response(rpc_message.id.clone(), result));
        }
        
        None
    }
    
    /// Create JSON-RPC success response
    fn create_success_response(&self, id: Option<Value>, result: Value) -> String {
        let response = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id,
            method: None,
            params: None,
            result: Some(result),
            error: None,
        };
        serde_json::to_string(&response).unwrap_or_default()
    }
    
    /// Create JSON-RPC error response
    fn create_error_response(&self, id: Option<Value>, code: i64, message: &str) -> String {
        let response = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id,
            method: None,
            params: None,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        };
        serde_json::to_string(&response).unwrap_or_default()
    }
    
    /// Send selection changed notification
    fn send_selection_changed(&self, selection: SelectionData) {
        log_debug!("Sending selection changed notification");
        
        let notification = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some("selection_changed".to_string()),
            params: Some(serde_json::to_value(selection).unwrap_or(Value::Null)),
            result: None,
            error: None,
        };
        
        let message = serde_json::to_string(&notification).unwrap_or_default();
        log_info!("Selection changed notification: {}", message);
        // In a real implementation, this would be sent to connected WebSocket clients
    }
    
    /// Send at-mention notification
    fn send_at_mention(&self, at_mention: AtMentionParams) {
        log_debug!("Sending at-mention notification");
        
        let notification = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some("at_mentioned".to_string()),
            params: Some(serde_json::to_value(at_mention).unwrap_or(Value::Null)),
            result: None,
            error: None,
        };
        
        let message = serde_json::to_string(&notification).unwrap_or_default();
        log_info!("At-mention notification: {}", message);
        // In a real implementation, this would be sent to connected WebSocket clients
    }

    /// Handle basic MCP tool calls (WASM-compatible, stubbed for MVP)
    fn handle_tool_call(&self, tool_name: &str, params: &Value) -> Value {
        log_debug!("MCP tool call '{}' with params: {}", tool_name, params);
        let result = match tool_name {
            "openFile" => {
                let file_path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Opening file: {}", file_path)
                    }]
                })
            }
            "getCurrentSelection" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": r#"{"success": true, "selection": {"text": "", "filePath": "", "isEmpty": true}}"#
                    }]
                })
            }
            "getWorkspaceFolders" => {
                let folders = self.get_workspace_folders();
                let folders_json: Vec<_> = folders
                    .iter()
                    .map(|f| {
                        serde_json::json!({
                            "name": f.split('/').last().unwrap_or("workspace"),
                            "uri": format!("file://{}", f),
                            "path": f
                        })
                    })
                    .collect();

                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&serde_json::json!({
                            "success": true,
                            "folders": folders_json,
                            "rootPath": folders.first().unwrap_or(&String::new())
                        })).unwrap_or_default()
                    }]
                })
            }
            "getOpenEditors" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": r#"{"success": true, "editors": []}"#
                    }]
                })
            }
            "openDiff" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Diff view opened"
                    }]
                })
            }
            "checkDocumentDirty" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": r#"{"success": true, "isDirty": false}"#
                    }]
                })
            }
            "saveDocument" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": r#"{"success": true, "saved": false}"#
                    }]
                })
            }
            "close_tab" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Tab closed"
                    }]
                })
            }
            "closeAllDiffTabs" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "All diff tabs closed"
                    }]
                })
            }
            "getDiagnostics" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": r#"{"success": true, "diagnostics": []}"#
                    }]
                })
            }
            "getLatestSelection" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": r#"{"success": true, "selection": null}"#
                    }]
                })
            }
            "executeCode" => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Code execution not supported in Zed"
                    }]
                })
            }
            _ => {
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Unknown tool: {}", tool_name)
                    }]
                })
            }
        };
        log_success!("MCP tool '{}' completed", tool_name);
        result
    }

}

zed_extension_api::register_extension!(ClaudeCodeExtension);
