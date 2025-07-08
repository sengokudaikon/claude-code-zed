use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MCPError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
    pub prompts: Option<PromptsCapability>,
    pub logging: Option<LoggingCapability>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoggingCapability {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub type_: String,
    pub text: String,
}

pub struct MCPServer {
    capabilities: ServerCapabilities,
}

impl MCPServer {
    pub fn new() -> Self {
        let capabilities = ServerCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(true),
            }),
            prompts: Some(PromptsCapability {
                list_changed: Some(false),
            }),
            logging: Some(LoggingCapability {}),
        };

        Self { capabilities }
    }

    pub async fn handle_request(&self, request: MCPRequest) -> Result<MCPResponse> {
        info!("Handling MCP request: {}", request.method);
        debug!("Request params: {:?}", request.params);

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await?,
            "tools/list" => self.handle_tools_list().await?,
            "tools/call" => self.handle_tools_call(request.params).await?,
            "logging/setLevel" => self.handle_logging_set_level(request.params).await?,
            "prompts/list" => self.handle_prompts_list().await?,
            "prompts/get" => self.handle_prompts_get(request.params).await?,
            _ => {
                return Ok(MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(MCPError {
                        code: -32601,
                        message: format!("Method not found: {}", request.method),
                        data: None,
                    }),
                });
            }
        };

        Ok(MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(result),
            error: None,
        })
    }

    async fn handle_initialize(&self, params: Option<Value>) -> Result<Value> {
        info!("Initializing MCP session");
        
        if let Some(params) = params {
            debug!("Initialize params: {}", params);
        }

        Ok(serde_json::json!({
            "protocolVersion": "2025-03-26",
            "capabilities": self.capabilities,
            "serverInfo": ServerInfo {
                name: "claude-code-server".to_string(),
                version: "0.1.0".to_string()
            }
        }))
    }

    async fn handle_tools_list(&self) -> Result<Value> {
        info!("Listing available tools");
        
        let tools = vec![
            Tool {
                name: "echo".to_string(),
                description: Some("Echo back the input text".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Text to echo back"
                        }
                    },
                    "required": ["text"]
                }),
            },
            Tool {
                name: "get_workspace_info".to_string(),
                description: Some("Get information about the current workspace".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "closeAllDiffTabs".to_string(),
                description: Some("Close all diff tabs in the editor".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "openFile".to_string(),
                description: Some("Open a file in the editor and optionally select a range of text".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "filePath": {
                            "type": "string",
                            "description": "Path to the file to open"
                        },
                        "preview": {
                            "type": "boolean",
                            "description": "Whether to open the file in preview mode",
                            "default": false
                        },
                        "startText": {
                            "type": "string",
                            "description": "Text pattern to find the start of the selection range. Selects from the beginning of this match."
                        },
                        "endText": {
                            "type": "string",
                            "description": "Text pattern to find the end of the selection range. Selects up to the end of this match. If not provided, only the startText match will be selected."
                        },
                        "selectToEndOfLine": {
                            "type": "boolean",
                            "description": "If true, selection will extend to the end of the line containing the endText match.",
                            "default": false
                        },
                        "makeFrontmost": {
                            "type": "boolean",
                            "description": "Whether to make the file the active editor tab. If false, the file will be opened in the background without changing focus.",
                            "default": true
                        }
                    },
                    "required": ["filePath"]
                }),
            },
            Tool {
                name: "getCurrentSelection".to_string(),
                description: Some("Get the current text selection in the active editor".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "getOpenEditors".to_string(),
                description: Some("Get information about currently open editors".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "getWorkspaceFolders".to_string(),
                description: Some("Get all workspace folders currently open in the IDE".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "openDiff".to_string(),
                description: Some("Open a git diff for the file".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "old_file_path": {
                            "type": "string",
                            "description": "Path to the file to show diff for. If not provided, uses active editor."
                        },
                        "new_file_path": {
                            "type": "string",
                            "description": "Path to the file to show diff for. If not provided, uses active editor."
                        },
                        "new_file_contents": {
                            "type": "string",
                            "description": "Contents of the new file. If not provided then the current file contents of new_file_path will be used."
                        },
                        "tab_name": {
                            "type": "string",
                            "description": "Path to the file to show diff for. If not provided, uses active editor."
                        }
                    },
                    "required": ["old_file_path", "new_file_path", "new_file_contents", "tab_name"]
                }),
            },
            Tool {
                name: "getLatestSelection".to_string(),
                description: Some("Get the most recent text selection (even if not in the active editor)".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "getDiagnostics".to_string(),
                description: Some("Get language diagnostics from VS Code".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "uri": {
                            "type": "string",
                            "description": "Optional file URI to get diagnostics for. If not provided, gets diagnostics for all files."
                        }
                    },
                    "required": []
                }),
            },
            Tool {
                name: "checkDocumentDirty".to_string(),
                description: Some("Check if a document has unsaved changes (is dirty)".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "filePath": {
                            "type": "string",
                            "description": "Path to the file to check"
                        }
                    },
                    "required": ["filePath"]
                }),
            },
            Tool {
                name: "saveDocument".to_string(),
                description: Some("Save a document with unsaved changes".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "filePath": {
                            "type": "string",
                            "description": "Path to the file to save"
                        }
                    },
                    "required": ["filePath"]
                }),
            },
            Tool {
                name: "close_tab".to_string(),
                description: Some("Close a tab by name".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "tab_name": {
                            "type": "string",
                            "description": "Name of the tab to close"
                        }
                    },
                    "required": ["tab_name"]
                }),
            },
            Tool {
                name: "executeCode".to_string(),
                description: Some("Execute python code in the Jupyter kernel for the current notebook file".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "The code to be executed on the kernel."
                        }
                    },
                    "required": ["code"]
                }),
            },
        ];

        Ok(serde_json::json!({
            "tools": tools
        }))
    }

    async fn handle_tools_call(&self, params: Option<Value>) -> Result<Value> {
        let params = params.ok_or_else(|| anyhow::anyhow!("Missing parameters for tools/call"))?;
        
        let tool_name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;

        let default_args = serde_json::json!({});
        let arguments = params.get("arguments").unwrap_or(&default_args);

        info!("Calling tool: {}", tool_name);
        debug!("Tool arguments: {}", arguments);

        let content = match tool_name {
            "echo" => {
                let text = arguments.get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No text provided");
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Echo: {}", text),
                }]
            }
            "get_workspace_info" => {
                let workspace_info = std::env::current_dir()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "Unknown workspace".to_string());
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Current workspace: {}", workspace_info),
                }]
            }
            "closeAllDiffTabs" => {
                info!("Closing all diff tabs");
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: "All diff tabs have been closed".to_string(),
                }]
            }
            "openFile" => {
                let file_path = arguments.get("filePath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No file path provided");
                let preview = arguments.get("preview")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let start_text = arguments.get("startText")
                    .and_then(|v| v.as_str());
                let end_text = arguments.get("endText")
                    .and_then(|v| v.as_str());
                
                info!("Opening file: {} (preview: {})", file_path, preview);
                
                let mut response = format!("Opened file: {}", file_path);
                if preview {
                    response.push_str(" (preview mode)");
                }
                if let Some(start) = start_text {
                    response.push_str(&format!(" with selection starting at '{}'", start));
                    if let Some(end) = end_text {
                        response.push_str(&format!(" ending at '{}'", end));
                    }
                }
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: response,
                }]
            }
            "getCurrentSelection" => {
                info!("Getting current selection");
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: "No text currently selected".to_string(),
                }]
            }
            "getOpenEditors" => {
                info!("Getting open editors");
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: "No editors currently open".to_string(),
                }]
            }
            "getWorkspaceFolders" => {
                let workspace_info = std::env::current_dir()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "Unknown workspace".to_string());
                
                info!("Getting workspace folders");
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Workspace folders: [{}]", workspace_info),
                }]
            }
            "openDiff" => {
                let old_file_path = arguments.get("old_file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No old file path provided");
                let new_file_path = arguments.get("new_file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No new file path provided");
                let tab_name = arguments.get("tab_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("diff");
                
                info!("Opening diff for {} vs {}", old_file_path, new_file_path);
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Opened diff view in tab '{}' comparing {} and {}", tab_name, old_file_path, new_file_path),
                }]
            }
            "getLatestSelection" => {
                info!("Getting latest selection");
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: "No recent text selection found".to_string(),
                }]
            }
            "getDiagnostics" => {
                let uri = arguments.get("uri")
                    .and_then(|v| v.as_str());
                
                info!("Getting diagnostics for: {:?}", uri);
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: if let Some(uri) = uri {
                        format!("No diagnostics found for {}", uri)
                    } else {
                        "No diagnostics found in workspace".to_string()
                    },
                }]
            }
            "checkDocumentDirty" => {
                let file_path = arguments.get("filePath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No file path provided");
                
                info!("Checking if document is dirty: {}", file_path);
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Document {} has no unsaved changes", file_path),
                }]
            }
            "saveDocument" => {
                let file_path = arguments.get("filePath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No file path provided");
                
                info!("Saving document: {}", file_path);
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Document {} saved successfully", file_path),
                }]
            }
            "close_tab" => {
                let tab_name = arguments.get("tab_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No tab name provided");
                
                info!("Closing tab: {}", tab_name);
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Tab '{}' has been closed", tab_name),
                }]
            }
            "executeCode" => {
                let code = arguments.get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No code provided");
                
                info!("Executing code: {}", code.chars().take(50).collect::<String>());
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Code executed successfully. Output: (simulated execution of {} characters)", code.len()),
                }]
            }
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        };

        Ok(serde_json::json!({
            "content": content,
            "isError": false
        }))
    }

    async fn handle_logging_set_level(&self, params: Option<Value>) -> Result<Value> {
        if let Some(params) = params {
            let level = params.get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            info!("Setting log level to: {}", level);
        }

        Ok(serde_json::json!({}))
    }

    async fn handle_prompts_list(&self) -> Result<Value> {
        info!("Listing available prompts");
        
        Ok(serde_json::json!({
            "prompts": []
        }))
    }

    async fn handle_prompts_get(&self, params: Option<Value>) -> Result<Value> {
        let params = params.ok_or_else(|| anyhow::anyhow!("Missing parameters for prompts/get"))?;
        
        let prompt_name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing prompt name"))?;

        info!("Getting prompt: {}", prompt_name);

        Ok(serde_json::json!({
            "description": format!("Prompt: {}", prompt_name),
            "messages": []
        }))
    }
}

impl Default for MCPServer {
    fn default() -> Self {
        Self::new()
    }
}