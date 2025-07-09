use zed_extension_api::{
    current_platform, download_file, latest_github_release, lsp::*, make_file_executable,
    Architecture, DownloadedFileType, GithubReleaseOptions, Os, *,
};

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
        // First try release binary, then debug binary
        let release_binary_path = format!("{}/target/release/claude-code-server", worktree_root);
        let debug_binary_path = format!("{}/target/debug/claude-code-server", worktree_root);

        // Check if release binary exists and is executable
        if std::path::Path::new(&release_binary_path).exists() {
            eprintln!(
                "🔍 [DEBUG] Using development release binary: {}",
                release_binary_path
            );
            return Ok(release_binary_path);
        }

        // Fall back to debug binary
        if std::path::Path::new(&debug_binary_path).exists() {
            eprintln!(
                "🔍 [DEBUG] Using development debug binary: {}",
                debug_binary_path
            );
            return Ok(debug_binary_path);
        }

        eprintln!("⚠️ [WARNING] No built binary found in target directory");
    }

    // For production: download binary from GitHub releases
    download_server_binary()
}

/// Download claude-code-server binary from GitHub releases
fn download_server_binary() -> Result<String, String> {
    const GITHUB_REPO: &str = "jiahaoxiang2000/claude-code-zed";

    eprintln!("🔍 [DEBUG] Starting GitHub release download process");

    // Determine platform-specific binary name
    let binary_name = match get_platform_binary_name() {
        Ok(name) => {
            eprintln!("🔍 [DEBUG] Platform binary name: {}", name);
            name
        }
        Err(e) => {
            eprintln!("❌ [ERROR] Failed to determine platform binary name: {}", e);
            return Err(e);
        }
    };

    // Get the latest release from GitHub
    eprintln!(
        "🔍 [DEBUG] Fetching latest release from GitHub repo: {}",
        GITHUB_REPO
    );
    let release = latest_github_release(
        GITHUB_REPO,
        GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        },
    )
    .map_err(|e| {
        eprintln!("❌ [ERROR] Failed to fetch GitHub release: {}", e);
        format!("Failed to get latest release: {}", e)
    })?;

    eprintln!(
        "📥 [INFO] Found release {} with {} assets",
        release.version,
        release.assets.len()
    );

    // Log all available assets for debugging
    eprintln!("🔍 [DEBUG] Available assets:");
    for asset in &release.assets {
        eprintln!("  - {}", asset.name);
    }

    // Find the asset that matches our platform
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == binary_name)
        .ok_or_else(|| {
            eprintln!("❌ [ERROR] Asset {} not found in release", binary_name);
            eprintln!("🔍 [DEBUG] Looking for asset matching: {}", binary_name);
            format!("Asset {} not found in release", binary_name)
        })?;

    eprintln!("✅ [SUCCESS] Found matching asset: {}", asset.name);
    eprintln!("🔍 [DEBUG] Download URL: {}", asset.download_url);

    // Download the binary to the extension's working directory
    let local_path = binary_name.clone();
    eprintln!("🔍 [DEBUG] Downloading to local path: {}", local_path);

    match download_file(
        &asset.download_url,
        &local_path,
        DownloadedFileType::Uncompressed,
    ) {
        Ok(_) => {
            eprintln!("✅ [SUCCESS] Binary downloaded to: {}", local_path);

            // Make the binary executable
            eprintln!("🔍 [DEBUG] Making binary executable: {}", local_path);
            make_file_executable(&local_path).map_err(|e| {
                eprintln!("❌ [ERROR] Failed to make binary executable: {}", e);
                format!("Failed to make binary executable: {}", e)
            })?;

            eprintln!("✅ [SUCCESS] Binary is now executable");
            Ok(local_path)
        }
        Err(e) => {
            eprintln!("❌ [ERROR] Failed to download binary: {}", e);
            eprintln!("🔍 [DEBUG] Download error details: {}", e);

            // Fallback to system PATH
            eprintln!("🔄 [FALLBACK] Using system binary: claude-code-server");
            Ok("claude-code-server".to_string())
        }
    }
}

/// Get platform-specific binary name for GitHub releases
fn get_platform_binary_name() -> Result<String, String> {
    // Use Zed's platform detection instead of env::consts which returns wasm32
    let (os, arch) = current_platform();

    match (os, arch) {
        (Os::Mac, Architecture::Aarch64) => Ok("claude-code-server-macos-aarch64".to_string()),
        (Os::Mac, Architecture::X8664) => Ok("claude-code-server-macos-x86_64".to_string()),
        (Os::Linux, Architecture::X8664) => Ok("claude-code-server-linux-x86_64".to_string()),
        (Os::Windows, _) => Err("Windows is not currently supported".to_string()),
        (os, arch) => Err(format!("Unsupported platform: {:?}-{:?}", os, arch)),
    }
}

zed_extension_api::register_extension!(ClaudeCodeExtension);
