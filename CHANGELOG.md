# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure and documentation

## [0.1.0] - 2025-01-09

### Added
- **Claude Code Zed Extension**: WebAssembly-based extension for Zed editor
- **Claude Code Server**: Native Rust server for WebSocket communication
- **GitHub Actions Workflow**: Automated cross-platform binary builds
- **HTTP Binary Download**: Automatic binary download from GitHub releases
- **LSP Integration**: Language Server Protocol bridge between Zed and Claude Code CLI
- **WebSocket Server**: Real-time communication with Claude Code CLI
- **Authentication**: Token-based authentication via lock files
- **Platform Support**: Linux x86_64, macOS x86_64, and macOS aarch64
- **Selection Tracking**: Real-time text selection changes sent to Claude Code
- **File Reference Handling**: Context sharing for selected code snippets
- **Development Mode**: Automatic detection of development environment
- **Fallback Support**: Graceful fallback to system PATH if download fails

### Features
- ✅ **Text Selection Sharing**: Zed can send selected text context to Claude Code CLI
- ✅ **File Reference Handling**: Selected code snippets and file paths are transmitted
- ✅ **WebSocket Communication**: Stable connection between Zed and Claude Code CLI
- ✅ **Authentication**: Secure token-based authentication via lock files
- ✅ **Cross-Platform**: Linux x86_64, macOS Intel/ARM support
- ✅ **Automatic Binary Management**: Extension handles binary download and caching

### Technical Details
- **Architecture**: Two-component system (WASM extension + native server)
- **Communication**: LSP ↔ WebSocket ↔ Claude Code CLI
- **Protocols**: JSON-RPC, WebSocket, MCP (Model Context Protocol)
- **Cache Directory**: `~/.claude-code-zed/` for binary storage
- **Port Range**: 10000-65535 for WebSocket server
- **Lock File**: `~/.claude/ide/[port].lock` for discovery

### Limitations
- **LSP Diagnostics**: Currently NOT implemented - doesn't expose IDE diagnostic information
- **One-way Communication**: Primary flow is Zed → Claude Code
- **Windows Support**: Not supported in this release

### Development
- **Workspace Structure**: Cargo workspace with extension and server packages
- **Build System**: GitHub Actions for automated releases
- **Testing**: Development mode with local binary detection
- **Documentation**: Comprehensive README and technical documentation

---

## Release Notes Format

Each release includes:
- **Cross-platform binaries**: Linux x86_64, macOS x86_64, macOS aarch64
- **Installation instructions**: How to install and configure the extension
- **Breaking changes**: Any API or configuration changes
- **Bug fixes**: Resolved issues and improvements
- **New features**: Added functionality and enhancements

For detailed technical documentation, see [README.md](README.md).