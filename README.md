# Claude Code Zed Integration

A two-part system that integrates Claude Code CLI with Zed editor for AI-assisted coding.

![File Selection Demo](docs/file-select.png)

## Current Integration Status

### ✅ Working Features
- **Text Selection Sharing**: Zed can send selected text context to Claude Code CLI
- **File Reference Handling**: Selected code snippets and file paths are transmitted
- **WebSocket Communication**: Stable connection between Zed and Claude Code CLI

### 🚧 Limitations
- **LSP Diagnostics**: Currently NOT implemented - Zed extension works as LSP client but doesn't expose IDE diagnostic information (errors, warnings, type hints) to Claude Code CLI
- **One-way Communication**: Primary flow is Zed → Claude Code; limited Claude Code → Zed capabilities

## Installation

### Prerequisites
- Zed editor
- Claude Code CLI

### Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/jiahaoxiang2000/claude-code-zed.git
   ```

2. **Install the Zed extension** (Development Mode):
   - Open Zed editor
   - Press `Cmd+Shift+P` (macOS) or `Ctrl+Shift+P` (Linux/Windows) to open the command palette
   - Type "zed: install dev extension" and select it
   - Navigate to and select the `claude-code-extension` folder in your cloned repository
   - The extension will be installed and activated automatically

3. **The claude-code-server is automatically downloaded**:
   - The extension will automatically download the appropriate `claude-code-server` binary from GitHub releases
   - No manual build or installation of the server is required
   - The server binary is cached in the extension's working directory

### Supported Platforms
- **macOS**: Intel (x86_64) and Apple Silicon (aarch64)
- **Linux**: x86_64
- **Windows**: Not currently supported

### Language Server Activation

The Claude Code extension runs as a Language Server Protocol (LSP) server and automatically activates when you open files with the following extensions:

- **Rust** (`.rs`)
- **JavaScript** (`.js`)
- **TypeScript** (`.ts`, `.tsx`)
- **Python** (`.py`)
- **Markdown** (`.md`)

#### Adding Support for Other File Types

To enable Claude Code integration for additional file types, edit the `claude-code-extension/extension.toml` file:

```toml
[language_servers.claude-code-server]
name = "Claude Code Server"
languages = ["Rust", "JavaScript", "TypeScript", "Python", "Markdown", "Go", "Java"]

[language_servers.claude-code-server.language_ids]
"Rust" = "rust"
"JavaScript" = "javascript"
"TypeScript" = "typescript"
"Python" = "python"
"Markdown" = "markdown"
"Go" = "go"
"Java" = "java"
```

After modifying the configuration, reinstall extention and restart Zed for the changes to take effect.

## Architecture Overview

This project consists of two components:

### 1. Zed Extension (`claude-code-zed`)
- **Purpose**: Zed editor integration and LSP communication
- **Technology**: Rust compiled to WebAssembly
- **Responsibilities**:
  - Editor selection tracking
  - File reference handling
  - LSP server lifecycle management
  - Communication with the companion server

### 2. Claude Code Server (`claude-code-server`)
- **Purpose**: WebSocket server for Claude Code CLI communication
- **Technology**: Native Rust application
- **Responsibilities**:
  - WebSocket server on localhost
  - Lock file management (`~/.claude/ide/[port].lock`)
  - Authentication token handling
  - JSON-RPC protocol implementation
  - Bridging between Zed extension and Claude Code CLI

## How It Works

The system implements the Claude Code protocol as documented in [claudecode.nvim PROTOCOL.md](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md):

### Communication Flow

1. **Zed Extension Startup**:
   - Extension loads in Zed's WASM environment
   - Establishes LSP connection to companion server
   - Begins tracking editor selections and file changes

2. **Companion Server Launch**:
   - `claude-code-server` starts as native process
   - Creates WebSocket server on random port (10000-65535)
   - Writes discovery lock file to `~/.claude/ide/[port].lock`
   - Sets environment variables (`CLAUDE_CODE_SSE_PORT`, `ENABLE_IDE_INTEGRATION`)

3. **Claude Code Discovery**:
   - Claude Code CLI discovers server via lock file
   - Authenticates using UUID token from lock file
   - Establishes WebSocket connection

4. **Bidirectional Communication**:
   - **Zed → Claude**: Selection changes, file references via LSP → Server → WebSocket
   - **Claude → Zed**: MCP tool calls via WebSocket → Server → LSP → Extension

### Component Interaction

```
Zed Editor (WASM)  ←→  LSP  ←→  Native Server  ←→  WebSocket  ←→  Claude Code CLI
     │                              │
     └── Selection tracking         └── Protocol implementation
     └── File references            └── Lock file management
     └── WASM-safe operations       └── Full system access
```

## Protocol Implementation

### Messages from Zed to Claude Code

- `selection_changed`: Notifies Claude when text selection changes
