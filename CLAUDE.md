# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Git Commit Convention

Follow Conventional Commits specification (https://www.conventionalcommits.org/):

**Format:** `<type>(<optional scope>): <description>`

**Allowed types:**
- `feat` - New feature
- `fix` - Bug fix  
- `docs` - Documentation changes
- `style` - Code style/formatting (no logic change)
- `refactor` - Code refactoring (no feature/fix)
- `perf` - Performance improvements
- `test` - Adding/updating tests
- `build` - Build system/dependency changes
- `ci` - CI/CD changes
- `chore` - Maintenance tasks
- `revert` - Revert a previous commit

**Examples:**
- `feat: add WebSocket port generation`
- `fix(server): resolve port binding conflicts`
- `docs: update installation instructions`
- `refactor(extension): simplify error handling`

**Rules:**
- First line should be <= 100 characters
- Use present tense ("add" not "added")
- Use imperative mood ("fix" not "fixes")
- Don't capitalize first letter after colon
- No period at the end of subject line

## Code Style Guidelines

### General Principles
- Write clean, maintainable, and self-documenting code
- Follow language-specific conventions and best practices
- Prioritize readability and clarity over cleverness
- Keep functions small and focused on a single responsibility

### Rust Code Style
- Follow standard Rust formatting (use `rustfmt`)
- Use descriptive variable and function names
- Prefer explicit error handling over panics
- Document public APIs with doc comments
- Use appropriate log levels (error, warn, info, debug, trace)

### Logging Guidelines
- Use structured logging without decorative elements
- Log levels:
  - `ERROR` - Critical failures requiring attention
  - `WARN` - Potential issues that don't stop execution
  - `INFO` - Important state changes and milestones
  - `DEBUG` - Detailed information for troubleshooting
  - `TRACE` - Very detailed execution flow
- Format: `[LEVEL] Component: message`
- Include relevant context in log messages

### Documentation
- Keep README focused on setup and usage
- Use DEVELOPMENT.md for technical implementation details
- Document architectural decisions and trade-offs
- Write clear, concise comments only when necessary
- Prefer self-documenting code over extensive comments

### Testing
- Write unit tests for business logic
- Include integration tests for critical paths
- Test error conditions and edge cases
- Maintain test coverage above 70% for core functionality

### Pull Request Guidelines
- One feature/fix per PR
- Include tests for new functionality
- Update documentation as needed
- Ensure CI passes before merging
- Use squash and merge for clean history
