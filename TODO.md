# TODO: Claude Code Zed Extension Development

## Phase 1: MVP (Minimum Viable Product)

### 🔧 Fix Compilation Issues
- [ ] **Resolve HTTP crate version conflicts**
  - Update Cargo.toml dependencies to use compatible versions
  - Fix `tokio-tungstenite` and `http` crate compatibility
  - Ensure all WebSocket types align with tungstenite API

- [ ] **Fix Zed Extension API compatibility**
  - Update Extension trait implementation methods
  - Fix `label_for_completion` vs `labels_for_completions` mismatch
  - Ensure all required trait methods are implemented correctly

- [ ] **Resolve WebSocket message type issues**
  - Fix `Message::Text` string vs `Utf8Bytes` type mismatch
  - Update WebSocket callback error handling
  - Ensure proper authentication callback signature

### 🏗️ Core MVP Implementation
- [ ] **Simplified WebSocket Server**
  - Create basic WebSocket server that binds to localhost
  - Implement minimal authentication (token validation)
  - Create and manage lock file in `~/.claude/ide/[port].lock`
  - Set required environment variables

- [ ] **Basic Protocol Support**
  - Implement JSON-RPC 2.0 message structure
  - Handle incoming connections from Claude Code CLI
  - Support basic `at_mentioned` message broadcasting
  - Minimal error handling and logging

- [ ] **Essential MCP Tools**
  - `openFile` - Basic file opening in Zed
  - `getCurrentSelection` - Return current text selection
  - `getWorkspaceFolders` - Return workspace information
  - Basic error responses for unsupported tools

### 🧪 MVP Testing & Validation
- [ ] **Compilation & Build**
  - `cargo check` passes without errors
  - `cargo build` completes successfully
  - Extension manifest is valid

- [ ] **Basic Integration Test**
  - WebSocket server starts and binds to port
  - Lock file is created correctly
  - Environment variables are set
  - Can accept WebSocket connections (mock test)

## Phase 2: Full Protocol Implementation

### 📡 Complete Protocol Support
- [ ] **Selection Tracking**
  - Implement `selection_changed` notifications
  - Track text selection changes in Zed
  - Broadcast selection updates to Claude Code

- [ ] **Advanced MCP Tools**
  - `openDiff` - Diff view support
  - `getOpenEditors` - List open tabs
  - `getDiagnostics` - Language server diagnostics
  - `saveDocument` - Document save operations
  - `executeCode` - Code execution (if applicable)

- [ ] **Error Handling & Resilience**
  - Robust connection handling
  - Graceful disconnection and cleanup
  - Proper error reporting to Claude Code
  - Server restart capabilities

### 🔍 Advanced Features
- [ ] **Authentication Enhancements**
  - Secure token generation and validation
  - Connection timeout handling
  - Multiple client support considerations

- [ ] **Performance Optimizations**
  - Efficient message broadcasting
  - Memory management for long-running sessions
  - WebSocket connection pooling if needed

## Phase 3: Production Readiness

### 📦 Extension Packaging
- [ ] **Zed Extension Standards**
  - Follow Zed extension best practices
  - Proper WASM compilation and optimization
  - Extension marketplace submission preparation

- [ ] **Documentation & Examples**
  - User installation guide
  - Development setup instructions
  - Troubleshooting documentation
  - API reference for supported tools

### 🚀 Release Preparation
- [ ] **Testing Suite**
  - Unit tests for core functionality
  - Integration tests with mock Claude Code
  - Manual testing with actual Claude Code CLI
  - Performance benchmarking

- [ ] **CI/CD Pipeline**
  - Automated builds and tests
  - Release automation
  - Version management

## Immediate Next Steps (MVP Focus)

1. **Start with dependency fixes** - Update Cargo.toml to resolve version conflicts
2. **Simplify the Extension trait implementation** - Remove complex features temporarily
3. **Create minimal working WebSocket server** - Focus on basic connectivity
4. **Test compilation at each step** - Ensure `cargo check` passes before adding features

## Success Criteria for MVP

✅ **MVP is complete when:**
- Extension compiles without errors (`cargo check` passes)
- WebSocket server starts and creates lock file
- Can accept basic connections (even if tools are stubbed)
- Environment variables are set correctly
- Basic project structure is established for further development

This MVP approach prioritizes getting a working foundation before implementing the full protocol specification.