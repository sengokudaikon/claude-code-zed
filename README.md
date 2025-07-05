# Claude Code Zed Extension

A Zed extension that integrates with Claude Code CLI for AI-assisted coding directly in your Zed editor.

## Features

- **WebSocket Integration**: Creates a WebSocket server that Claude Code can connect to
- **Selection Tracking**: Sends selection changes to Claude Code in real-time
- **At-Mention Support**: Send file references and code selections to Claude Code
- **Lock File Management**: Automatically manages the discovery lock file for Claude Code
- **Authentication**: Secure token-based authentication for Claude Code connections

## Installation

1. Clone this repository
2. Build the extension:
   ```bash
   cargo build --release
   ```
3. Install in Zed by adding to your extensions directory

## How It Works

This extension implements the Claude Code protocol as documented in the [claudecode.nvim PROTOCOL.md](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md):

1. **WebSocket Server**: Creates a WebSocket server on a random port (10000-65535)
2. **Lock File**: Writes a discovery file to `~/.claude/ide/[port].lock` with connection details
3. **Environment Variables**: Sets `CLAUDE_CODE_SSE_PORT` and `ENABLE_IDE_INTEGRATION`
4. **Authentication**: Uses UUID-based token authentication via WebSocket headers
5. **Message Protocol**: Implements JSON-RPC 2.0 over WebSocket for bidirectional communication

## Protocol Implementation

### Messages from Zed to Claude Code

- `selection_changed`: Notifies Claude when text selection changes
- `at_mentioned`: Sends file references and code selections to Claude

### Messages from Claude Code to Zed

The extension implements the following MCP tools:

- `openFile`: Open files in Zed editor
- `getCurrentSelection`: Get current text selection
- `getWorkspaceFolders`: Get workspace folder information
- `getOpenEditors`: Get list of open editor tabs

## Configuration

The extension automatically:
- Detects workspace folders
- Generates secure authentication tokens
- Manages the WebSocket server lifecycle
- Handles Claude Code connection state

## Security

- WebSocket server binds to localhost (127.0.0.1) only
- Uses UUID-based authentication tokens
- Validates authentication headers on connection

## Development

This extension is built with:
- Rust and WebAssembly for the core logic
- Zed Extension API for editor integration
- Serde for JSON serialization
- UUID for authentication token generation

### Debugging & Logs

The extension includes comprehensive logging for development:

```bash
# Run Zed in foreground mode to see extension logs
zed --foreground
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test with Zed
5. Submit a pull request

## License

MIT License - see LICENSE file for details
