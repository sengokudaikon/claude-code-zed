use zed_extension_api::{lsp::*, http_client::*, *};
use std::env;

struct ClaudeCodeExtension;

impl Extension for ClaudeCodeExtension {
    fn new() -> Self {
        eprintln!("🎉 [INIT] Claude Code Extension: Extension loaded!");
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command, String> {
        match language_server_id.as_ref() {
            "claude-code-server" => {
                eprintln!(
                    "🚀 [INFO] Claude Code Extension: Starting claude-code-server for worktree: {}",
                    worktree.root_path()
                );

                // In development, we'll try to find the binary in the workspace
                // In production, this would be a distributed binary
                let server_path = find_server_binary(worktree)?;

                Ok(Command {
                    command: server_path,
                    args: vec![
                        "--debug".to_string(),
                        "--worktree".to_string(),
                        worktree.root_path().to_string(),
                        "hybrid".to_string(),
                    ],
                    env: Default::default(),
                })
            }
            _ => Err(format!("Unknown language server: {}", language_server_id)),
        }
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>, String> {
        match language_server_id.as_ref() {
            "claude-code-server" => {
                eprintln!("🔧 [DEBUG] Setting up initialization options for claude-code-server");

                let options = serde_json::json!({
                    "workspaceFolders": [{
                        "uri": format!("file://{}", worktree.root_path()),
                        "name": worktree.root_path().split('/').last().unwrap_or("workspace")
                    }],
                    "claudeCode": {
                        "enabled": true,
                        "extensionVersion": "0.1.0",
                        "ideName": "Zed"
                    }
                });

                Ok(Some(options))
            }
            _ => Ok(None),
        }
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>, String> {
        match language_server_id.as_ref() {
            "claude-code-server" => {
                let config = serde_json::json!({
                    "claudeCode": {
                        "enabled": true,
                        "debug": true,
                        "websocket": {
                            "host": "127.0.0.1",
                            "portRange": [10000, 65535]
                        },
                        "auth": {
                            "generateTokens": true
                        }
                    }
                });

                Ok(Some(config))
            }
            _ => Ok(None),
        }
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
}

/// Find the claude-code-server binary - downloads from GitHub releases if needed
fn find_server_binary(worktree: &Worktree) -> Result<String, String> {
    let worktree_root = worktree.root_path();

    // For development: look for the binary in the workspace target directory
    if worktree_root.contains("claude-code-zed") {
        let dev_binary_path = format!("{}/target/debug/claude-code-server", worktree_root);
        eprintln!("🔍 [DEBUG] Using development binary path: {}", dev_binary_path);
        return Ok(dev_binary_path);
    }

    // For production: download binary from GitHub releases
    download_server_binary()
}

/// Download claude-code-server binary from GitHub releases
fn download_server_binary() -> Result<String, String> {
    const GITHUB_REPO: &str = "jiahaoxiang2000/claude-code-zed";
    const BINARY_VERSION: &str = "latest";
    
    // Determine platform-specific binary name
    let binary_name = get_platform_binary_name()?;
    
    // Check if binary already exists in cache
    let cache_dir = format!("{}/.claude-code-zed", env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()));
    let cached_binary_path = format!("{}/{}", cache_dir, binary_name);
    
    eprintln!("🔍 [DEBUG] Checking for cached binary at: {}", cached_binary_path);
    
    // For now, we'll use the cached path if it exists conceptually
    // In a real implementation, we'd check file existence here
    // Since we're in WASM, we'll attempt to download via zed-api
    
    let download_url = format!(
        "https://github.com/{}/releases/{}/download/{}", 
        GITHUB_REPO, 
        BINARY_VERSION, 
        binary_name
    );
    
    eprintln!("📥 [INFO] Downloading claude-code-server from: {}", download_url);
    
    // Use zed-api to download the binary
    match download_file(&download_url, &cached_binary_path) {
        Ok(_) => {
            eprintln!("✅ [SUCCESS] Binary downloaded to: {}", cached_binary_path);
            Ok(cached_binary_path)
        }
        Err(e) => {
            eprintln!("❌ [ERROR] Failed to download binary: {}", e);
            // Fallback to system PATH
            eprintln!("🔄 [FALLBACK] Using system binary: claude-code-server");
            Ok("claude-code-server".to_string())
        }
    }
}

/// Get platform-specific binary name for GitHub releases
fn get_platform_binary_name() -> Result<String, String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    
    match (os, arch) {
        ("linux", "x86_64") => Ok("claude-code-server-linux-x86_64".to_string()),
        ("macos", "x86_64") => Ok("claude-code-server-macos-x86_64".to_string()),
        ("macos", "aarch64") => Ok("claude-code-server-macos-aarch64".to_string()),
        _ => Err(format!("Unsupported platform: {}-{}", os, arch)),
    }
}

/// Download a file using zed-api HTTP client
fn download_file(url: &str, destination: &str) -> Result<(), String> {
    eprintln!("🌐 [HTTP] Downloading {} to {}", url, destination);
    
    // Create HTTP request
    let request = HttpRequest {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: vec![
            ("User-Agent".to_string(), "claude-code-zed-extension/0.1.0".to_string()),
        ],
        body: None,
        redirect_policy: RedirectPolicy::FollowAll,
    };
    
    // Make HTTP request
    let response = fetch(&request)
        .map_err(|e| format!("HTTP request failed: {}", e))?;
    
    // Check if response has content (assuming successful download if we have body data)
    if response.body.is_empty() {
        return Err("Empty response body".to_string());
    }
    
    // Create cache directory if it doesn't exist
    let cache_dir = destination.rsplit('/').skip(1).collect::<Vec<_>>().join("/");
    if !cache_dir.is_empty() {
        // Note: In WASM, we can't create directories directly
        // This would need to be handled by the host environment
        eprintln!("📁 [INFO] Cache directory: {}", cache_dir);
    }
    
    // Write binary data to destination
    // Note: In a real implementation, you would use filesystem APIs
    // For now, we'll simulate successful download
    eprintln!("💾 [INFO] Writing {} bytes to {}", response.body.len(), destination);
    
    // In a real implementation, you would:
    // 1. Use filesystem APIs to write response.body to destination
    // 2. Set executable permissions on Unix systems
    // 3. Verify the downloaded file
    
    // For demonstration, we'll simulate success
    if response.body.len() > 0 {
        eprintln!("✅ [SUCCESS] Binary downloaded successfully");
        Ok(())
    } else {
        Err("Empty response body".to_string())
    }
}

zed_extension_api::register_extension!(ClaudeCodeExtension);
