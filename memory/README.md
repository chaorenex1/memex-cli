# Memex Memory Plugin for Claude Code

**Seamless memory and context management for Claude Code through memex-cli integration**

[![Version](https://img.shields.io/badge/version-1.0.0-blue.svg)](CHANGELOG.md)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Overview

The memory plugin brings persistent memory capabilities to Claude Code through automated hooks that:
- **Search and inject** relevant past knowledge into conversations automatically
- **Record and persist** tool usage and session knowledge for future reference
- **Manage session lifecycle** with initialization and cleanup
- **Provide manual tools** for explicit memory operations

Perfect for projects requiring context continuity across sessions, knowledge retention, and collaborative memory sharing.

## Features

### 🔍 Automatic Memory Retrieval
- Searches memory service when you submit prompts
- Injects top relevant results into conversation context
- Configurable search parameters (limit, relevance threshold)
- Transparent operation - Claude references past knowledge naturally

### 📝 Automatic Knowledge Recording
- Captures Write, Edit, and Bash tool usage automatically
- Extracts question-answer pairs from tool operations
- Records complete session transcripts
- Quality gating to filter sensitive or trivial content

### 🛠️ Manual Memory Tools
- `/setup-memex` - Interactive configuration wizard
- `/test-memory` - Test connectivity and functionality
- `/validate-hooks` - Validate configuration and dependencies
- `/view-memory-logs` - View and analyze hook execution logs

### 📚 Comprehensive Skills
- **memory-usage** - Guide for using memory features effectively
- **hook-troubleshooting** - Debug and resolve hook issues

## Quick Start

### Prerequisites

1. **memex-cli** installed and accessible:
   ```bash
   cd /path/to/memex_cli
   cargo build --release -p memex-cli
   # Add to PATH or note full path
   ```

2. **Memory service** running (HTTP endpoint)

3. **Python 3.8+** with dependencies:
   ```bash
   pip install -r memory/requirements-http.txt
   ```

### Installation

#### Option 1: Claude Code Plugin System (Recommended)

```bash
# Install from marketplace (when available)
claude plugin install memory

# Or install locally
claude plugin install /path/to/memory
```

#### Option 2: Manual Installation

```bash
# Copy to Claude Code plugins directory
cp -r memory ~/.claude/plugins/

# Or use project-specific installation
# (place in project root, Claude Code auto-discovers)
```

### Configuration

Run the interactive setup wizard:

```
/setup-memex
```

This creates `.claude/memory.local.md` with your settings:
- Memory service URL
- API key (if required)
- Search parameters
- memex-cli path

**Alternative**: Manually create `.claude/memory.local.md`:

```markdown
---
enabled: true
base_url: "http://localhost:8080"
api_key: "your-api-key"
search_limit: 5
min_score: 0.6
memex_cli_path: "memex-cli"
---

# Memex Memory Configuration

Project-specific settings.
```

### Verify Installation

```
/test-memory
```

This validates:
- memex-cli accessibility
- Memory service connectivity
- Search and record functionality
- Python dependencies
- Hook scripts

**Important**: Restart Claude Code after configuration changes.

## Usage

### Automatic Memory Features

**Memory Injection** (automatic on every prompt):
1. You ask a question
2. Hook searches memory service
3. Relevant past knowledge injected into context
4. Claude references this knowledge in response

**Knowledge Recording** (automatic on tool use):
1. Claude executes Write/Edit/Bash tool
2. Hook extracts what was done and why
3. Records as candidate knowledge
4. Future prompts can retrieve this knowledge

**Session Recording** (automatic on task completion):
1. Claude finishes a task
2. Hook parses conversation transcript
3. Extracts key Q&A pairs
4. Saves complete session for future reference

### Manual Memory Operations

**Search memory**:
```bash
memex-cli search --query "JWT authentication" --limit 5
```

**Record knowledge**:
```bash
memex-cli record-candidate \
  --query "How to implement rate limiting" \
  --answer "Use token bucket algorithm with Redis" \
  --tags "backend,optimization"
```

**View logs**:
```
/view-memory-logs
```

### Configuration Options

Edit `.claude/memory.local.md`:

| Setting | Description | Default |
|---------|-------------|---------|
| `enabled` | Enable/disable hooks | `true` |
| `base_url` | Memory service URL | `http://localhost:8080` |
| `api_key` | Authentication key | From global config |
| `search_limit` | Max search results | `5` |
| `min_score` | Min relevance (0-1) | `0.6` |
| `memex_cli_path` | Path to executable | `memex-cli` |

See [SETTINGS_TEMPLATE.md](SETTINGS_TEMPLATE.md) for detailed documentation.

## Architecture

```
memory/
├── .claude-plugin/
│   └── plugin.json          # Plugin manifest
├── skills/
│   ├── memory-usage/        # Memory usage guide
│   └── hook-troubleshooting/ # Debug guide
├── commands/
│   ├── setup-memex.md       # Configuration wizard
│   ├── test-memory.md       # Connectivity tests
│   ├── validate-hooks.md    # Configuration validation
│   └── view-memory-logs.md  # Log viewer
├── hooks/
│   └── hooks.json           # Hook event configuration
└── scripts/
    ├── memory-inject.py     # Search and inject
    ├── memory-record.py     # Record tool usage
    ├── session-init.py      # Session initialization
    ├── session-cleanup.py   # Session cleanup
    └── ...                  # Additional utilities
```

### Hook Events

| Event | Script | Purpose |
|-------|--------|---------|
| SessionStart | session-init.py | Initialize session state |
| UserPromptSubmit | memory-inject.py | Search and inject memory |
| PostToolUse | memory-record.py | Record tool usage |
| Stop/SubagentStop | record-session-enhanced.py | Record complete session |
| PreCompact | record-session-enhanced.py | Preserve context |
| SessionEnd | session-cleanup.py | Clean up resources |

## Troubleshooting

### Hooks Not Triggering

**Symptom**: No memory injection or recording

**Solutions**:
1. Check `enabled: true` in `.claude/memory.local.md`
2. Restart Claude Code (hooks load at session start)
3. Validate configuration: `/validate-hooks`
4. Check logs: `/view-memory-logs`

### memex-cli Not Found

**Symptom**: "command not found: memex-cli"

**Solutions**:
1. Add to PATH: `export PATH="$PATH:/path/to/memex-cli"`
2. Or specify full path in settings:
   ```yaml
   memex_cli_path: "/full/path/to/memex-cli"
   ```

### Connection Failed

**Symptom**: "Connection refused" errors

**Solutions**:
1. Start memory service
2. Verify `base_url` in configuration
3. Test connectivity: `curl http://localhost:8080/health`

### Python Import Errors

**Symptom**: "ModuleNotFoundError: No module named 'requests'"

**Solution**:
```bash
pip install -r scripts/requirements-http.txt
```

**For comprehensive troubleshooting**, load the skill:
```
Ask Claude: "troubleshoot memory hooks"
```

Or run diagnostics:
```
/validate-hooks
/test-memory
/view-memory-logs
```

## Configuration Examples

### Minimal (Default Settings)

```yaml
---
enabled: true
---
```

### Standard Development

```yaml
---
enabled: true
base_url: "http://localhost:8080"
search_limit: 5
min_score: 0.6
---
```

### Research Mode (Broad Context)

```yaml
---
enabled: true
search_limit: 15
min_score: 0.5
---
```

### Strict Matching (High Precision)

```yaml
---
enabled: true
search_limit: 3
min_score: 0.8
---
```

## Best Practices

### Memory Management

1. **Trust automation**: Let hooks capture knowledge automatically
2. **Tune parameters**: Adjust `search_limit` and `min_score` based on your needs
3. **Review logs**: Periodically check `/view-memory-logs` for issues
4. **Curate strategically**: Manually record critical knowledge not captured automatically
5. **Clean logs**: Archive old logs to manage disk space

### Performance

- Use lower `search_limit` (3-5) for faster responses
- Increase `min_score` (0.7-0.8) for more relevant results
- Monitor log file sizes and clean periodically
- Consider disabling hooks for unrelated projects

### Security

- Add `.claude/*.local.md` to `.gitignore` (plugin does this automatically)
- Don't commit API keys or sensitive configuration
- Use project-specific settings for team collaboration
- Review recorded knowledge for sensitive content

## Integration with memex-cli

This plugin requires [memex-cli](https://github.com/yourusername/memex-cli) and a compatible memory service.

**Global Configuration** (`~/.memex/config.toml`):
```toml
[memory]
base_url = "http://localhost:8080"
api_key = "your-api-key"
timeout_ms = 30000

[memory.search]
limit = 5
min_score = 0.6
```

**Project Configuration** (`.claude/memory.local.md`):
- Overrides global settings per project
- Allows per-project memory service URLs
- Enables/disables hooks without affecting other projects

## Contributing

For bug reports and feature requests, open an issue on GitHub.

## License

Apache License 2.0 - see [LICENSE](LICENSE) file for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and release notes.

## Support

- **Documentation**: Load skills with `/` commands
- **Diagnostics**: Run `/test-memory` and `/validate-hooks`
- **Logs**: View with `/view-memory-logs`
- **Issues**: Report on GitHub

## Credits

Built for the memex-cli ecosystem by the Memex CLI Team.

Designed to integrate seamlessly with Claude Code's hook system for transparent, powerful memory capabilities.

---

**Version**: 1.0.0
**Last Updated**: 2026-01-12
**Status**: Production Ready
