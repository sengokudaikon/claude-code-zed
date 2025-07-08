use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error};

// Tool error codes
pub const TOOL_ERROR_INVALID_PARAMS: i32 = -32602;
pub const TOOL_ERROR_INTERNAL: i32 = -32603;
pub const TOOL_ERROR_NOT_FOUND: i32 = -32601;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub struct ToolHandler {
    pub schema: ToolSchema,
    pub handler: fn(&Value) -> Result<Value, ToolError>,
    pub requires_async: bool,
}

impl std::fmt::Debug for ToolHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolHandler")
            .field("schema", &self.schema)
            .field("handler", &"<function>")
            .field("requires_async", &self.requires_async)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl ToolError {
    pub fn new(code: i32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }
    
    pub fn with_data(code: i32, message: String, data: Value) -> Self {
        Self {
            code,
            message,
            data: Some(data),
        }
    }
    
    pub fn invalid_params(message: String) -> Self {
        Self::new(TOOL_ERROR_INVALID_PARAMS, message)
    }
    
    pub fn internal_error(message: String) -> Self {
        Self::new(TOOL_ERROR_INTERNAL, message)
    }
    
    pub fn not_found(name: String) -> Self {
        Self::new(TOOL_ERROR_NOT_FOUND, format!("Tool not found: {}", name))
    }
}

#[derive(Debug)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolHandler>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
    
    pub fn register_tool(&mut self, handler: ToolHandler) {
        debug!("Registering tool: {}", handler.schema.name);
        self.tools.insert(handler.schema.name.clone(), handler);
    }
    
    pub fn get_tool_list(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|h| h.schema.clone()).collect()
    }
    
    pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
        let handler = self.tools.get(name)
            .ok_or_else(|| ToolError::not_found(name.to_string()))?;
        
        debug!("Calling tool: {} with args: {:?}", name, args);
        
        match (handler.handler)(args) {
            Ok(result) => {
                debug!("Tool {} completed successfully", name);
                Ok(result)
            }
            Err(e) => {
                error!("Tool {} failed: {:?}", name, e);
                Err(e)
            }
        }
    }
    
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Tool implementations
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    
    // Register openFile tool
    registry.register_tool(ToolHandler {
        schema: ToolSchema {
            name: "openFile".to_string(),
            description: "Opens a file in the editor".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to open"
                    }
                },
                "required": ["path"]
            }),
        },
        handler: handle_open_file,
        requires_async: false,
    });
    
    // Register getCurrentSelection tool
    registry.register_tool(ToolHandler {
        schema: ToolSchema {
            name: "getCurrentSelection".to_string(),
            description: "Gets the current text selection in the editor".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        handler: handle_get_current_selection,
        requires_async: false,
    });
    
    // Register getOpenEditors tool
    registry.register_tool(ToolHandler {
        schema: ToolSchema {
            name: "getOpenEditors".to_string(),
            description: "Gets a list of currently open editors".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        handler: handle_get_open_editors,
        requires_async: false,
    });
    
    // Register saveDocument tool
    registry.register_tool(ToolHandler {
        schema: ToolSchema {
            name: "saveDocument".to_string(),
            description: "Saves a document with the given content".to_string(),
            input_schema: serde_json::json!({
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
            }),
        },
        handler: handle_save_document,
        requires_async: false,
    });
    
    // Register getWorkspaceFolders tool
    registry.register_tool(ToolHandler {
        schema: ToolSchema {
            name: "getWorkspaceFolders".to_string(),
            description: "Gets the current workspace folders".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        handler: handle_get_workspace_folders,
        requires_async: false,
    });
    
    registry
}

// Tool handler implementations
fn handle_open_file(args: &Value) -> Result<Value, ToolError> {
    let path = args.get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| ToolError::invalid_params("Missing path parameter".to_string()))?;
    
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(serde_json::json!({
            "path": path,
            "content": content
        })),
        Err(e) => Err(ToolError::internal_error(format!("Failed to read file: {}", e))),
    }
}

fn handle_get_current_selection(_args: &Value) -> Result<Value, ToolError> {
    Ok(serde_json::json!({
        "selection": "",
        "path": "",
        "line": 0,
        "column": 0
    }))
}

fn handle_get_open_editors(_args: &Value) -> Result<Value, ToolError> {
    Ok(serde_json::json!({
        "editors": []
    }))
}

fn handle_save_document(args: &Value) -> Result<Value, ToolError> {
    let path = args.get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| ToolError::invalid_params("Missing path parameter".to_string()))?;
    
    let content = args.get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| ToolError::invalid_params("Missing content parameter".to_string()))?;
    
    match std::fs::write(path, content) {
        Ok(_) => Ok(serde_json::json!({
            "path": path,
            "saved": true
        })),
        Err(e) => Err(ToolError::internal_error(format!("Failed to save file: {}", e))),
    }
}

fn handle_get_workspace_folders(_args: &Value) -> Result<Value, ToolError> {
    // This would typically get workspace folders from the current working directory
    // For now, return the current directory
    match std::env::current_dir() {
        Ok(cwd) => Ok(serde_json::json!({
            "folders": [cwd.to_string_lossy().to_string()]
        })),
        Err(_) => Ok(serde_json::json!({
            "folders": []
        })),
    }
}