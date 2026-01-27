# Memex Memory Plugin for Claude Code

**Seamless memory and context management for Claude Code through memex-cli integration**

[![Version](https://img.shields.io/badge/version-1.1.0-blue.svg)](CHANGELOG.md)
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

### 🛠️ Quick Commands
| Command | Purpose |
|---------|---------|
| `/setup` | Interactive configuration wizard |
| `/test` | Quick connectivity test |
| `/health` | Full system diagnostics |
| `/logs` | View recent hook logs |
| `/search` | Manual memory search |
| `/record` | Manually record knowledge |
| `/reset` | Clean up / reset state |

### 📚 Skills
| Skill | Purpose |
|-------|---------|
| **memory-workflow** | Complete memory usage guide |
| **debugging** | Systematic troubleshooting |
| **optimization** | Performance tuning guide |

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
/setup
```

This creates or updates `~/.memex/config.toml` (global) or `./config.toml` (project) with your settings:
- Memory provider (service/local/hybrid)
- Service URL and API key
- Search parameters
- Additional sections (prompt_inject, gatekeeper, candidate_extract, events_out)

**Alternative**: Manually edit `~/.memex/config.toml`:

```toml
[memory]
provider = "service"
enabled = true
base_url = "http://localhost:8080"
api_key = ""
search_limit = 6
min_score = 0.2
timeout_ms = 10000
```

### Verify Installation

```
/test
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
```
/search
```

**Record knowledge**:
```
/record
```

**View logs**:
```
/logs
```

### Configuration Options

Edit `~/.memex/config.toml` (global) or `./config.toml` (project):

| Setting | Section | Description | Default |
|---------|---------|-------------|---------|
| `provider` | [memory] | Memory backend | `service` |
| `enabled` | [memory] | Enable/disable memory | `true` |
| `base_url` | [memory] | Memory service URL | `http://localhost:8080` |
| `api_key` | [memory] | Authentication key | `""` |
| `search_limit` | [memory] | Max search results | `6` |
| `min_score` | [memory] | Min relevance (0-1) | `0.2` |
| `timeout_ms` | [memory] | Request timeout | `10000` |
| `max_items` | [prompt_inject] | Max items to inject | `10` |
| `min_level_inject` | [gatekeeper] | Min validation level | `2` |
| `max_candidates` | [candidate_extract] | Max candidates to extract | `10` |
| `enabled` | [events_out] | Log tool events | `true` |

## Architecture

```
memory/
├── .claude-plugin/
│   └── plugin.json          # Plugin manifest
├── skills/
│   ├── memory-workflow/     # Memory usage guide
│   ├── debugging/           # Troubleshooting guide
│   └── optimization/        # Performance tuning
├── commands/
│   ├── setup.md             # Configuration wizard
│   ├── test.md              # Connectivity tests
│   ├── health.md            # Full diagnostics
│   ├── logs.md              # Log viewer
│   ├── search.md            # Manual search
│   ├── record.md            # Manual record
│   └── reset.md             # Reset / cleanup
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

### Memory Not Working

**Symptom**: No memory injection or recording

**Solutions**:
1. Check `enabled = true` in `[memory]` section of config.toml
2. Verify config is valid TOML: `python -c "import tomllib; tomllib.load(open('config.toml', 'rb'))"`
3. Run diagnostics: `/health`
4. Check logs: `/logs`

### memex-cli Not Found

**Symptom**: "command not found: memex-cli"

**Solutions**:
1. Build memex-cli: `cargo build --release -p memex-cli`
2. Add to PATH: `export PATH="$PATH:/path/to/memex_cli/target/release"`

### Connection Failed

**Symptom**: "Connection refused" errors

**Solutions**:
1. Start memory service
2. Verify `base_url` in `[memory]` section of config.toml
3. Test connectivity: `curl ${base_url}/health`

### Config Parse Errors

**Symptom**: "Invalid TOML" errors

**Solutions**:
1. Validate TOML syntax
2. Check for proper section headers like `[memory]`
3. Verify string values are quoted

**For comprehensive troubleshooting**, load the debugging skill or run:
```
/health    # Full diagnostics
/test      # Quick connectivity check
/logs      # View recent errors
```

## Configuration Examples

### Minimal (Default Settings)

```toml
[memory]
enabled = true
```

### Standard Development

```toml
[memory]
enabled = true
base_url = "http://localhost:8080"
search_limit = 6
min_score = 0.2
```

### Research Mode (Broad Context)

```toml
[memory]
enabled = true
search_limit = 15
min_score = 0.1
```

### Strict Matching (High Precision)

```toml
[memory]
enabled = true
search_limit = 3
min_score = 0.5
```

## Best Practices

### Memory Management

1. **Trust automation**: Let hooks capture knowledge automatically
2. **Tune parameters**: Adjust `search_limit` and `min_score` based on your needs
3. **Review logs**: Periodically check `/logs` for issues
4. **Curate strategically**: Manually record critical knowledge not captured automatically
5. **Clean logs**: Archive old logs to manage disk space

### Performance

- Use lower `search_limit` (3-5) for faster responses
- Increase `min_score` (0.7-0.8) for more relevant results
- Monitor log file sizes and clean periodically
- Consider disabling hooks for unrelated projects

### Security

- Add `config.toml` to `.gitignore` (contains API keys)
- Don't commit API keys or sensitive configuration
- Use `redact = true` in `[candidate_extract]` to redact sensitive info
- Use `strict_secret_block = true` to block secrets completely
- Review recorded knowledge for sensitive content

## Integration with memex-cli

This plugin requires [memex-cli](https://github.com/yourusername/memex-cli) and a compatible memory service.

**Configuration Priority** (highest to lowest):
1. `./config.toml` - Project-specific configuration
2. `~/.memex/config.toml` - Global user configuration
3. Built-in defaults

**Complete Configuration Example**:
```toml
[memory]
provider = "service"
enabled = true
base_url = "http://localhost:8080"
api_key = ""
timeout_ms = 10000
search_limit = 6
min_score = 0.2

[prompt_inject]
placement = "user"
max_items = 10
max_answer_chars = 1000
include_meta_line = true

[gatekeeper]
provider = "standard"
max_inject = 10
min_trust_show = 0.40
min_level_inject = 2

[candidate_extract]
max_candidates = 10
max_answer_chars = 2000
min_answer_chars = 100
redact = true

[events_out]
enabled = true
path = "./run.events.jsonl"
```

## Contributing

For bug reports and feature requests, open an issue on GitHub.

## License

Apache License 2.0 - see [LICENSE](LICENSE) file for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and release notes.

## Support

- **Documentation**: Load skills with `/` commands
- **Diagnostics**: Run `/test` and `/health`
- **Logs**: View with `/logs`
- **Issues**: Report on GitHub

## Credits

Built for the memex-cli ecosystem by the Memex CLI Team.

Designed to integrate seamlessly with Claude Code's hook system for transparent, powerful memory capabilities.

---

**Version**: 1.1.0
**Last Updated**: 2026-01-27
**Status**: Production Ready
