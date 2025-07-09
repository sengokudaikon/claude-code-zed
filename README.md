# Claude Code Zed Integration

A two-part system that integrates Claude Code CLI with Zed editor for AI-assisted coding.

## Current Integration Status

### ✅ Working Features
- **Text Selection Sharing**: Zed can send selected text context to Claude Code CLI
- **File Reference Handling**: Selected code snippets and file paths are transmitted
- **WebSocket Communication**: Stable connection between Zed and Claude Code CLI

### 🚧 Limitations
- **LSP Diagnostics**: Currently NOT implemented - Zed extension works as LSP client but doesn't expose IDE diagnostic information (errors, warnings, type hints) to Claude Code CLI
- **One-way Communication**: Primary flow is Zed → Claude Code; limited Claude Code → Zed capabilities

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

## Installation

### Prerequisites
- Rust toolchain installed
- Zed editor
- Claude Code CLI

### Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/your-repo/claude-code-zed
   cd claude-code-zed
   ```

2. **Build the Zed extension**:
   ```bash
   cargo build --release
   ```

3. **Install companion server** (separate repository):
   ```bash
   git clone https://github.com/your-repo/claude-code-server
   cd claude-code-server
   cargo install --path .
   ```

4. **Install Zed extension**:
   - Copy the built extension to your Zed extensions directory
   - Or use Zed's extension manager (when available)

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
