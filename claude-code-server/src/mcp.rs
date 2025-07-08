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
                description: Some("Open a file in the editor".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "filePath": {
                            "type": "string",
                            "description": "Path to the file to open"
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
                
                info!("Opening file: {}", file_path);
                
                vec![TextContent {
                    type_: "text".to_string(),
                    text: format!("Opened file: {}", file_path),
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