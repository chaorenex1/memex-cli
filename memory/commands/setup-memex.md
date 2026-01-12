---
name: setup-memex
description: Interactive configuration wizard for memory plugin
allowed-tools: ["AskUserQuestion", "Write", "Read", "Bash"]
---

# Setup Memex Configuration

Create or update `.claude/memory.local.md` configuration file through interactive wizard.

## Steps

### 1. Check Existing Configuration

Read existing configuration if present:
```
Read .claude/memory.local.md
```

If file exists, parse current settings and inform user of existing values.

### 2. Gather Configuration via AskUserQuestion

Collect settings through user prompts:

**Question 1: Memory Service URL**
- Ask: "What is your memory service URL?"
- Options:
  - "http://localhost:8080 (default)"
  - "http://localhost:9000 (alternative port)"
  - "Custom URL"
- If custom, prompt for URL input

**Question 2: API Authentication**
- Ask: "Do you have an API key for the memory service?"
- Options:
  - "Yes, I have an API key"
  - "No, service doesn't require authentication"
- If yes, prompt for API key (use secure input)

**Question 3: Search Parameters**
- Ask: "How many search results should be injected automatically?"
- Options:
  - "3 (minimal context)"
  - "5 (balanced - recommended)"
  - "10 (comprehensive)"
  - "Custom number"

**Question 4: Relevance Threshold**
- Ask: "Minimum relevance score for search results? (0.0-1.0)"
- Options:
  - "0.8 (very strict)"
  - "0.6 (balanced - recommended)"
  - "0.5 (permissive)"
  - "Custom value"

**Question 5: memex-cli Path**
- Ask: "Is memex-cli in your PATH, or do you need to specify a custom location?"
- Options:
  - "It's in PATH (use 'memex-cli')"
  - "Specify custom path"
- If custom, prompt for full path

### 3. Create Configuration File

Write settings to `.claude/memory.local.md`:

```markdown
---
enabled: true
base_url: "<user_provided_url>"
api_key: "<user_provided_key or omit if none>"
search_limit: <user_provided_limit>
min_score: <user_provided_score>
memex_cli_path: "<user_provided_path or 'memex-cli'>"
---

# Memex Memory Configuration

Created: <current_date>

This file controls memory plugin behavior for this project.

## Settings

- **enabled**: Controls whether hooks are active
- **base_url**: Memory service API endpoint
- **api_key**: Authentication key (keep secret)
- **search_limit**: Max results for automatic injection
- **min_score**: Minimum relevance threshold (0.0-1.0)
- **memex_cli_path**: Path to memex-cli executable

## Notes

After editing this file, restart Claude Code for changes to take effect.

To temporarily disable hooks: set `enabled: false`
```

### 4. Add to .gitignore

Ensure `.claude/*.local.md` is in .gitignore:

```bash
# Check if .gitignore exists
if [ -f .gitignore ]; then
  # Check if pattern already present
  if ! grep -q "\.claude/\*\.local\.md" .gitignore; then
    echo ".claude/*.local.md" >> .gitignore
  fi
else
  # Create .gitignore
  echo ".claude/*.local.md" > .gitignore
fi
```

### 5. Verify memex-cli Installation

Test if memex-cli is accessible:

```bash
<memex_cli_path> --version
```

If command fails:
- Inform user memex-cli is not accessible at specified path
- Suggest:
  - Install memex-cli: `cd /path/to/memex_cli && cargo build --release -p memex-cli`
  - Add to PATH or use full path in settings

### 6. Test Memory Service Connection

Test service connectivity:

```bash
curl -s <base_url>/health
```

If successful:
- Inform user service is reachable
- Proceed to test search

If failed:
- Warn user service is not responding
- Suggest:
  - Start memory service
  - Check firewall settings
  - Verify base_url is correct

### 7. Summary and Next Steps

Inform user:

```
✅ Configuration created at: .claude/memory.local.md

Settings summary:
- Service URL: <base_url>
- Search limit: <limit> results
- Min score: <score>
- memex-cli: <path>

⚠️  IMPORTANT: Restart Claude Code for hooks to load new configuration.

Next steps:
1. Exit this Claude Code session
2. Restart: claude
3. Test setup: /test-memory
4. Validate hooks: /validate-hooks

For troubleshooting, use: /view-memory-logs
```

## Edge Cases

- If user cancels any question → abort and inform them
- If custom values are invalid (e.g., min_score > 1.0) → validate and ask again
- If .claude directory doesn't exist → create it first
- If settings file exists → ask if user wants to overwrite or update specific fields

## Tips

- Use clear, non-technical language in questions
- Provide sensible defaults
- Validate user input before writing
- Test connectivity before finishing
- Always remind about restart requirement
