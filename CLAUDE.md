# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Building the Extension
```bash
# Check for compilation errors
cargo check
```

## Architecture Overview

This is a **Zed editor extension** that implements the Claude Code protocol for AI-assisted coding. The extension is built in Rust and compiles to WebAssembly for execution within Zed.

### Core Components

**ClaudeCodeExtension** - Main extension struct that implements the Zed Extension trait. Manages the lifecycle of the WebSocket server and handles extension initialization.

**ClaudeCodeServer** - WebSocket server implementation that:
- Binds to localhost on a random port (10000-65535)
- Creates authentication tokens using UUID v4
- Manages the lock file in `~/.claude/ide/[port].lock`
- Handles bidirectional JSON-RPC 2.0 communication with Claude Code CLI

**Protocol Implementation** - The extension implements the exact protocol specification from [claudecode.nvim](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md):
- **Discovery**: Uses lock files and environment variables (`CLAUDE_CODE_SSE_PORT`, `ENABLE_IDE_INTEGRATION`)
- **Authentication**: WebSocket header-based token validation (`x-claude-code-ide-authorization`)
- **Message Types**: `selection_changed`, `at_mentioned` (Zed → Claude), and MCP tool calls (Claude → Zed)

### Key Data Structures

- **LockFileData**: JSON structure for the discovery lock file
- **JsonRpcMessage**: JSON-RPC 2.0 message wrapper for all communication
- **SelectionData/AtMentionParams**: Selection and file reference data structures
- **MCP Tool Handlers**: `openFile`, `getCurrentSelection`, `getWorkspaceFolders`, `getOpenEditors`

### WebSocket Communication Flow

1. Extension starts WebSocket server on random port
2. Creates lock file with connection details and auth token
3. Sets environment variables for Claude Code discovery
4. Claude Code connects and authenticates via WebSocket headers
5. Bidirectional JSON-RPC communication established
6. Selection changes and file references are broadcast to Claude
7. Claude can call MCP tools to interact with Zed editor

### Security Considerations

- WebSocket server **must** bind to localhost (127.0.0.1) only
- Authentication tokens are UUID v4 generated per session
- Lock files contain process IDs for cleanup
- All file operations should validate paths and permissions

## Git Commit Convention

- Use emoji first to indicate commit type:
  - 🎉 `:tada:` - Initial commit or major feature
  - ✨ `:sparkles:` - New feature
  - 🐛 `:bug:` - Bug fix
  - 🔧 `:wrench:` - Configuration changes
  - 📝 `:memo:` - Documentation
  - 🚀 `:rocket:` - Performance improvements
  - 🎨 `:art:` - Code style/formatting
  - ♻️ `:recycle:` - Refactoring
  - 🔥 `:fire:` - Remove code/files
  - 📦 `:package:` - Add dependencies/submodules
