# Claude Code Zed Integration

A two-part system that integrates Claude Code CLI with Zed editor for AI-assisted coding.

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

## Features

- **Seamless Integration**: Works within Zed's WASM extension environment
- **WebSocket Communication**: Native server handles Claude Code protocol
- **Selection Tracking**: Real-time selection changes sent to Claude Code
- **At-Mention Support**: File references and code selections forwarded to Claude
- **Secure Authentication**: Token-based authentication between components
- **Protocol Compliance**: Full Claude Code protocol implementation

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
- `at_mentioned`: Sends file references and code selections to Claude

## Development

### Debugging & Logs

Both components include comprehensive logging:

```bash
# Run Zed in foreground mode to see extension logs
zed --foreground

# Run companion server with debug logging
RUST_LOG=debug claude-code-server
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test with Zed
5. Submit a pull request

### Architecture Benefits

This approach solves the WASM limitations by:
- **WASM Extension**: Handles editor integration within Zed's sandbox
- **Native Server**: Provides full system access for Claude Code protocol
- **LSP Bridge**: Enables secure communication between components
- **Separation of Concerns**: Each component focuses on its strengths

## License

MIT License - see LICENSE file for details
