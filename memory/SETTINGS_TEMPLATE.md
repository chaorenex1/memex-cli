# Settings Template for memory Plugin

Create this file at `.claude/memory.local.md` in your project to customize plugin behavior.

## Template

```markdown
---
enabled: true
base_url: "http://localhost:8080"
api_key: "your-api-key-here"
search_limit: 5
min_score: 0.6
memex_cli_path: "memex-cli"
---

# Memex Memory Configuration

Project-specific settings for memory plugin.

## Settings Explanation

- **enabled**: Set to `false` to disable hooks without removing configuration
- **base_url**: Memory service API endpoint (overrides ~/.memex/config.toml)
- **api_key**: Authentication key for memory service
- **search_limit**: Maximum search results for automatic injection (1-50)
- **min_score**: Minimum relevance threshold for search results (0.0-1.0)
- **memex_cli_path**: Path to memex-cli executable (use full path if not in PATH)

## Notes

After editing this file, restart Claude Code for changes to take effect.
```

## Field Details

### enabled (boolean)

**Default**: `true`

Controls whether hooks are active. Set to `false` to temporarily disable without removing configuration.

**Examples**:
```yaml
enabled: true   # Hooks active
enabled: false  # Hooks disabled
```

### base_url (string)

**Default**: From `~/.memex/config.toml` or `"http://localhost:8080"`

Memory service API endpoint. Must be full URL with protocol.

**Examples**:
```yaml
base_url: "http://localhost:8080"     # Local development
base_url: "http://localhost:9000"     # Alternative port
base_url: "https://memory.example.com" # Remote service
```

### api_key (string, optional)

**Default**: From `~/.memex/config.toml` or none

Authentication key for memory service. Omit if service doesn't require auth.

**Examples**:
```yaml
api_key: "sk-1234567890abcdef"  # With auth
# api_key: ""                    # No auth (comment out or omit)
```

### search_limit (integer)

**Default**: `5`

**Range**: 1-50

Maximum number of search results injected into conversation context.

**Recommendations**:
- **1-3**: Minimal context, fastest
- **5-10**: Balanced (recommended)
- **10-20**: Comprehensive context
- **20+**: Deep research (may slow down)

**Examples**:
```yaml
search_limit: 3   # Minimal
search_limit: 5   # Balanced
search_limit: 10  # Comprehensive
```

### min_score (float)

**Default**: `0.6`

**Range**: 0.0-1.0

Minimum relevance score for search results. Higher = stricter matching.

**Recommendations**:
- **0.8-1.0**: Very strict, high precision
- **0.6-0.8**: Balanced (recommended)
- **0.4-0.6**: Permissive, broader context
- **0.0-0.4**: Very permissive, may include noise

**Examples**:
```yaml
min_score: 0.8  # Strict
min_score: 0.6  # Balanced
min_score: 0.5  # Permissive
```

### memex_cli_path (string)

**Default**: `"memex-cli"`

Path to memex-cli executable. Use `"memex-cli"` if in PATH, otherwise provide full path.

**Examples**:
```yaml
# In PATH
memex_cli_path: "memex-cli"

# Custom path (Linux/macOS)
memex_cli_path: "/usr/local/bin/memex-cli"
memex_cli_path: "/home/user/projects/memex_cli/target/release/memex-cli"

# Custom path (Windows)
memex_cli_path: "C:\\Users\\user\\memex_cli\\target\\release\\memex-cli.exe"
```

## Example Configurations

### Minimal Setup

```yaml
---
enabled: true
---
```

Uses all defaults from global config.

### Standard Development

```yaml
---
enabled: true
base_url: "http://localhost:8080"
search_limit: 5
min_score: 0.6
---
```

### Strict Matching

```yaml
---
enabled: true
search_limit: 3
min_score: 0.8
---
```

Only highly relevant results, minimal context injection.

### Research Mode

```yaml
---
enabled: true
search_limit: 15
min_score: 0.5
---
```

Broader context, more results.

### Custom Installation

```yaml
---
enabled: true
memex_cli_path: "/custom/path/to/memex-cli"
base_url: "https://memory.company.com"
api_key: "company-api-key-123"
---
```

## Setup Wizard

Instead of manual creation, use the interactive wizard:

```
/setup-memex
```

The wizard will guide you through all settings and create the file automatically.

## Troubleshooting

**Settings not taking effect**:
- Restart Claude Code (hooks load at session start)
- Verify file location: `.claude/memory.local.md` in project root
- Check YAML syntax: no tabs, proper spacing

**Invalid YAML errors**:
```bash
# Validate syntax
cat .claude/memory.local.md | \
  awk '/^---$/{i++; next} i==1' | \
  python -c "import yaml,sys; yaml.safe_load(sys.stdin)"
```

**Can't find memex-cli**:
- Set full path in `memex_cli_path`
- Or add to PATH and restart terminal/Claude Code

## Security

**Important**: Add to `.gitignore`:

```
.claude/*.local.md
```

This prevents committing API keys and project-specific settings to version control.

## Migration

### From Global Config Only

If you only have `~/.memex/config.toml`, create minimal `.local.md`:

```yaml
---
enabled: true
---
```

### From Previous Hook System

If migrating from manual `.claude/settings.json` hooks:

1. Create `.claude/memory.local.md` with your settings
2. Remove manual hook entries from `.claude/settings.json`
3. Install memory plugin
4. Restart Claude Code

## Validation

After creating settings file:

```
/validate-hooks
```

This checks:
- YAML syntax
- Field validity
- memex-cli accessibility
- Service connectivity
