# Development Guide

This guide explains how to develop and contribute to the Claude Code Zed integration project locally.

## Project Structure

```
claude-code-zed/
├── claude-code-extension/          # Zed extension (Rust → WASM)
│   ├── src/
│   │   └── lib.rs                 # Extension implementation
│   ├── Cargo.toml                 # Extension dependencies
│   └── extension.toml             # Zed extension configuration
├── claude-code-server/            # Companion server (Native Rust)
│   ├── src/
│   │   ├── main.rs               # Server entry point
│   │   ├── lsp.rs                # LSP implementation
│   │   ├── mcp.rs                # MCP protocol handling
│   │   └── websocket.rs          # WebSocket server
│   └── Cargo.toml                # Server dependencies
├── README.md                      # User documentation
├── DEVELOPMENT.md                 # This file
└── Cargo.toml                     # Workspace configuration
```

## Development Environment Setup

### Prerequisites

1. **Rust Toolchain**: Install the latest stable Rust
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **WebAssembly Target**: Required for building the Zed extension
   ```bash
   rustup target add wasm32-wasip1
   ```

3. **Zed Editor**: Download from [zed.dev](https://zed.dev)

4. **Claude Code CLI**: Install from [claude.ai/code](https://claude.ai/code)

### Clone and Setup

```bash
# Clone the repository
git clone https://github.com/jiahaoxiang2000/claude-code-zed.git
cd claude-code-zed

# Build the entire workspace
cargo build
```

## Development Workflow

### 1. Extension Development

The Zed extension is written in Rust and compiled to WebAssembly.

#### Building the Extension

```bash
# Build the extension
cd claude-code-extension
cargo build

# The built extension will be in target/wasm32-wasip1/debug/
```

#### Installing for Development

Zed has built-in support for installing development extensions directly from source code, which automatically handles the build process:

1. **Install the extension using Zed's dev extension feature**:
   - Open Zed
   - Press `Cmd+Shift+P` (macOS) or `Ctrl+Shift+P` (Linux)
   - Type "zed: install dev extension" and select it
   - Navigate to and select the `claude-code-extension` folder
   - Zed will automatically build and install the extension

2. **View extension logs**:
   - Open Zed's log panel: `View → Debug` → `Open Log`
   - Extension logs will appear with `[EXTENSION]` prefix

#### Extension Development Tips

- **Auto-building**: Zed's dev extension installer automatically builds the WASM extension
- **Hot Reloading**: After making code changes, reinstall the extension using the same process
- **No manual build needed**: You don't need to run `cargo build` manually - Zed handles it
- **Logging**: Use `eprintln!()` for debugging - logs appear in Zed's debug panel
- **WASM Limitations**: The extension runs in a sandboxed WASM environment with limited system access

### 2. Server Development

The companion server is a native Rust application that handles WebSocket communication.

#### Building the Server

```bash
# Build the server
cd claude-code-server
cargo build

# For release build
cargo build --release
```

#### Running the Server Standalone

```bash
# Run in debug mode
cd claude-code-server
cargo run -- --debug --worktree /path/to/your/project hybrid

# Or run the built binary
./target/debug/claude-code-server --debug --worktree /path/to/your/project hybrid
```

#### Server Development Tips

- **Debugging**: Use `RUST_LOG=debug` for verbose logging
- **WebSocket Testing**: Use tools like `wscat` to test WebSocket connections
- **Lock Files**: Check `~/.claude/ide/` for server discovery files

### 3. Testing the Integration

#### End-to-End Testing

1. **Install extension in Zed** using the dev extension feature:
   - Open Zed
   - Press `Cmd+Shift+P` (macOS) or `Ctrl+Shift+P` (Linux)
   - Type "zed: install dev extension" and select it
   - Navigate to and select the `claude-code-extension` folder
   - Zed will automatically build and install the extension

2. **Test with Claude Code CLI**:
   ```bash
   # Open a supported file in Zed
   zed test.rs

   # In another terminal, run Claude Code CLI
   claude-code
   ```

3. **Verify connection**:
   - Check Zed logs for extension startup messages
   - Check `~/.claude/ide/` for lock files
   - Verify WebSocket connection in server logs
   - The server binary will be automatically downloaded when needed

## Architecture Deep Dive

### Extension Architecture

The Zed extension (`claude-code-extension`) is responsible for:

1. **LSP Server Management**: Starts and manages the companion server
2. **Binary Download**: Downloads platform-specific server binaries from GitHub releases
3. **Configuration**: Passes workspace and configuration data to the server

Key files:
- `src/lib.rs`: Main extension implementation
- `extension.toml`: Zed extension configuration

### Server Architecture

The companion server (`claude-code-server`) handles:

1. **WebSocket Server**: Creates WebSocket server on localhost
2. **Discovery Protocol**: Writes lock files for Claude Code CLI discovery
3. **Authentication**: Generates and validates UUID tokens
4. **Protocol Bridge**: Translates between LSP and Claude Code protocols

Key files:
- `src/main.rs`: Server entry point and argument parsing
- `src/lsp.rs`: LSP server implementation
- `src/websocket.rs`: WebSocket server and protocol handling
- `src/mcp.rs`: MCP (Model Context Protocol) implementation

### Communication Flow

```
1. Zed Extension starts → 2. Launches companion server → 3. Server creates WebSocket
                                                                    ↓
6. Claude Code CLI ← 5. Discovers via lock file ← 4. Writes discovery lock file
```

## Common Development Tasks

### Adding New Language Support

1. **Update extension configuration**:
   ```toml
   # claude-code-extension/extension.toml
   [language_servers.claude-code-server]
   languages = ["Rust", "JavaScript", "TypeScript", "Python", "Markdown", "NewLanguage"]

   [language_servers.claude-code-server.language_ids]
   "NewLanguage" = "newlanguage"
   ```

2. **Rebuild and reinstall** the extension

### Debugging Connection Issues

1. **Check extension logs** in Zed's debug panel
2. **Verify server startup** with manual server launch
3. **Check lock files** in `~/.claude/ide/`
4. **Test WebSocket connection** with `wscat`

### Adding New Protocol Messages

1. **Define message types** in `claude-code-server/src/mcp.rs`
2. **Implement handlers** in the WebSocket server
3. **Update LSP bridge** to forward messages
4. **Test with Claude Code CLI**

## Troubleshooting

### Common Issues

1. **Extension won't install**:
   - Check WASM target is installed: `rustup target add wasm32-wasip1`
   - Verify Cargo.toml has correct crate-type: `["cdylib"]`

2. **Server download fails**:
   - Check internet connection
   - Verify GitHub release assets exist
   - Check platform detection logic

3. **WebSocket connection fails**:
   - Check if port is available
   - Verify lock file permissions
   - Check firewall settings

## Contributing

### Code Style

- Follow Rust standard formatting: `cargo fmt`
- Run clippy for linting: `cargo clippy`
- Write tests for new functionality
- Document public APIs

## Resources

- [Zed Extension API Documentation](https://docs.rs/zed_extension_api/)
- [Claude Code Protocol Documentation](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md)
- [WebAssembly Rust Book](https://rustwasm.github.io/docs/book/)
- [LSP Specification](https://microsoft.github.io/language-server-protocol/)
