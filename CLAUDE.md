# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Building the Extension
```bash
# Check for compilation errors
cargo check

# Build for WASM target (configured as default in .cargo/config.toml)
cargo build --release
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

## Known Limitations

- WASM incompatible extension run limitation: The WebAssembly compilation may restrict certain native Rust functionalities, potentially impacting the extension's full feature set

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
