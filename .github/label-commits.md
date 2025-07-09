# Commit Labeling Guide

This document explains how GitHub release notes will be categorized based on commit messages.

## Emoji to Category Mapping

Based on the commit conventions in `CLAUDE.md`, GitHub will automatically categorize commits:

| Emoji | Type | GitHub Category |
|-------|------|----------------|
| 🎉 `:tada:` | Initial commit or major feature | **Exciting New Features 🎉** |
| ✨ `:sparkles:` | New feature | **Exciting New Features 🎉** |
| 🐛 `:bug:` | Bug fix | **Bug Fixes 🐛** |
| 🔧 `:wrench:` | Configuration changes | **Configuration Changes 🔧** |
| 📝 `:memo:` | Documentation | **Documentation 📝** |
| 🚀 `:rocket:` | Performance improvements | **Performance Improvements 🚀** |
| 🎨 `:art:` | Code style/formatting | **Code Style & Refactoring 🎨** |
| ♻️ `:recycle:` | Refactoring | **Code Style & Refactoring 🎨** |
| 🔥 `:fire:` | Remove code/files | **Other Changes** |
| 📦 `:package:` | Add dependencies/submodules | **Dependencies 📦** |

## How It Works

1. **Commit Message**: When you commit with an emoji prefix (e.g., `✨ feat: add new feature`)
2. **Release Notes**: GitHub automatically categorizes it under "Exciting New Features 🎉"
3. **Manual Labels**: You can also add labels to PRs to override categorization

## Example Release Notes Output

```markdown
## What's Changed

### Exciting New Features 🎉
* ✨ feat: implement GitHub Actions build and automatic binary download by @user

### Bug Fixes 🐛
* 🐛 fix: correct GitHub Actions workflow build paths by @user

### Configuration Changes 🔧
* 🔧 config: remove Windows support from releases by @user

### Documentation 📝
* 📝 docs: add comprehensive CHANGELOG.md by @user
```

## Best Practices

1. **Use consistent emojis** from the CLAUDE.md convention
2. **Add GitHub labels** to PRs for better categorization
3. **Write clear commit messages** that explain the change
4. **Use breaking-change label** for major API changes