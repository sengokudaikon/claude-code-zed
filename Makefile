# Claude Code Zed Development Makefile
# Simplifies building and deploying the server for development

# Detect platform for correct binary naming
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)

ifeq ($(UNAME_S),Darwin)
    ifeq ($(UNAME_M),arm64)
        BINARY_NAME = claude-code-server-macos-aarch64
    else
        BINARY_NAME = claude-code-server-macos-x86_64
    endif
    ZED_EXT_DIR = $(HOME)/Library/Application Support/Zed/extensions/work/claude-code-zed
else ifeq ($(UNAME_S),Linux)
    BINARY_NAME = claude-code-server-linux-x86_64
    ZED_EXT_DIR = $(HOME)/.local/share/zed/extensions/work/claude-code-zed
else
    $(error Unsupported platform: $(UNAME_S))
endif

.PHONY: dev-build dev-clean dev-test help all

all: help

dev-build: ## Build and deploy server for development
	@echo "🔨 Building claude-code-server in release mode..."
	@cd claude-code-server && cargo build --release
	@echo "📁 Creating Zed extension directory if it doesn't exist..."
	@mkdir -p "$(ZED_EXT_DIR)"
	@echo "📦 Copying binary to Zed extension directory..."
	@cp target/release/claude-code-server "$(ZED_EXT_DIR)/$(BINARY_NAME)"
	@echo "✅ Development build deployed successfully!"
	@echo "💡 Restart Zed to use the updated binary with your changes"
	@echo "📍 Binary deployed to: $(ZED_EXT_DIR)/$(BINARY_NAME)"

dev-clean: ## Remove development deployment
	@echo "🧹 Cleaning development deployment..."
	@if [ -f "$(ZED_EXT_DIR)/$(BINARY_NAME)" ]; then \
		rm -f "$(ZED_EXT_DIR)/$(BINARY_NAME)" && \
		echo "✅ Development deployment cleaned"; \
	else \
		echo "ℹ️  No development deployment found to clean"; \
	fi

dev-test: dev-build ## Build, deploy and show test instructions
	@echo ""
	@echo "🧪 Testing Instructions:"
	@echo "1. Restart Zed editor"
	@echo "2. Open a file with emojis (test_emoji.md exists in this repo)"
	@echo "3. Select text containing emojis like: 'Mixed content: abc 🎉 def 🚀 ghi'"
	@echo "4. Start Claude Code CLI: claude-code"
	@echo "5. Connect to IDE: /ide"
	@echo "6. Select claude-code-server from the menu"
	@echo "7. Ask 'what have i selected?' to verify UTF-16 fix is working"
	@echo ""
	@echo "✅ If you can select emoji text without server crashes, the fix is working!"

dev-debug: ## Build debug version and deploy
	@echo "🔨 Building claude-code-server in debug mode..."
	@cd claude-code-server && cargo build
	@echo "📁 Creating Zed extension directory if it doesn't exist..."
	@mkdir -p "$(ZED_EXT_DIR)"
	@echo "📦 Copying debug binary to Zed extension directory..."
	@cp target/debug/claude-code-server "$(ZED_EXT_DIR)/$(BINARY_NAME)"
	@echo "✅ Development debug build deployed successfully!"
	@echo "💡 Restart Zed to use the debug binary (larger, with debug symbols)"

status: ## Show current development deployment status
	@echo "🔍 Development Deployment Status:"
	@echo "Platform: $(UNAME_S) $(UNAME_M)"
	@echo "Binary name: $(BINARY_NAME)"
	@echo "Extension directory: $(ZED_EXT_DIR)"
	@if [ -f "$(ZED_EXT_DIR)/$(BINARY_NAME)" ]; then \
		echo "✅ Development binary exists"; \
		ls -la "$(ZED_EXT_DIR)/$(BINARY_NAME)"; \
	else \
		echo "❌ No development binary found"; \
		echo "Run 'make dev-build' to deploy a development build"; \
	fi

help: ## Show available commands
	@echo "Claude Code Zed Development Commands:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Quick start: make dev-build && restart Zed"