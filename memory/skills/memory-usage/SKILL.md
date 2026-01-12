---
name: Memory Usage Guide
description: This skill should be used when the user asks to "use memory features", "search memory", "record knowledge", "retrieve history", "inject context", "how to use memex memory", or mentions memory search/retrieval functionality.
version: 1.0.0
---

# Memory Usage Guide

This skill provides guidance on using the memory plugin's memory search, recording, and context injection capabilities.

## Overview

The memory plugin integrates memex-cli's memory service with Claude Code, providing:
- **Automatic memory retrieval**: Searches relevant past conversations when user submits prompts
- **Automatic knowledge recording**: Captures tool usage and extracts knowledge automatically
- **Session memory**: Records entire sessions for future reference
- **Manual memory operations**: Direct access to search and record commands

## Automatic Memory Features

### Memory Injection (UserPromptSubmit Hook)

**When it activates**: Every time you submit a prompt to Claude

**What it does**:
1. Extracts your question/prompt
2. Searches memex memory service for relevant past knowledge
3. Injects top matching results into conversation context
4. Claude sees this context and can reference it in responses

**Example flow**:
```
You: "How do I implement JWT authentication?"

[Hook automatically runs]
→ Searches memory for "JWT authentication"
→ Finds relevant past conversations
→ Injects as context:

### 📚 Related Memory
**[qa-12345]** Q: JWT best practices
A: Use httpOnly cookies, rotate tokens every 15 min...
Relevance: 0.85
---

Claude: Based on previous experience [qa-12345], I recommend...
```

**Configuration**: Controlled by search parameters in `.claude/memory.local.md` (see Settings section below)

### Knowledge Recording (PostToolUse Hook)

**When it activates**: After Claude executes Write, Edit, or Bash tools

**What it does**:
1. Captures tool usage details (command, file path, content)
2. Extracts query (what problem was solved) and answer (how it was solved)
3. Calls `memex-cli record-candidate` to save as candidate knowledge
4. Gatekeeper later evaluates and promotes candidates to verified memory

**Recorded tools**:
- **Write**: File creation with content
- **Edit**: File modifications
- **Bash**: Command execution (skips simple commands like `ls`, `pwd`)

**Example**:
```
Claude executes: Write tool creates auth.ts with JWT implementation

[Hook automatically runs]
→ Extracts query: "Implement JWT authentication"
→ Extracts answer: "Created auth.ts with token generation and validation"
→ Records as candidate: memex-cli record-candidate --query "..." --answer "..."
```

### Session Recording (Stop/SubagentStop/PreCompact Hooks)

**When it activates**: When Claude completes tasks, subagents finish, or context compacts

**What it does**:
1. Reads full conversation transcript
2. Extracts key questions and answers
3. Batch records all knowledge to memory service
4. Preserves session history for future retrieval

## Manual Memory Operations

### Search Memory

Use memex-cli directly to search for specific knowledge:

```bash
memex-cli search --query "JWT authentication" --limit 5 --min-score 0.6
```

**Parameters**:
- `--query`: Search text (required)
- `--limit`: Maximum results (default: 5)
- `--min-score`: Minimum relevance score 0-1 (default: 0.6)
- `--format`: Output format (json, markdown, plain)

**When to use**:
- Need specific past knowledge not automatically injected
- Want to verify what's stored in memory
- Testing memory service connectivity

### Record Knowledge

Manually record important knowledge:

```bash
# Record as candidate (needs validation)
memex-cli record-candidate \
  --query "How to implement rate limiting" \
  --answer "Use token bucket algorithm with Redis" \
  --tags "backend,rate-limiting,redis"

# Record as hit (directly verified)
memex-cli record-hit \
  --qa-id "qa-12345" \
  --hit-type "manual_verification"
```

**When to use**:
- Want to preserve specific knowledge explicitly
- Recording external knowledge (from documentation, team discussions)
- Promoting important candidates to verified status

### Record Session

Manually trigger session recording:

```bash
memex-cli record-session \
  --session-id "current-session-id" \
  --transcript-path "/path/to/transcript.txt"
```

**When to use**:
- Session didn't auto-record (hook failed)
- Want to record external conversation logs
- Testing session recording functionality

## Configuration

### Settings File

Create `.claude/memory.local.md` in your project to customize behavior:

```markdown
---
enabled: true
base_url: "http://localhost:8080"
api_key: "your-api-key"
search_limit: 10
min_score: 0.5
memex_cli_path: "memex-cli"
---

# Memex Memory Configuration

Custom settings for this project.
```

**Available settings**:
- `enabled`: Enable/disable hooks (true/false)
- `base_url`: Memory service URL (overrides ~/.memex/config.toml)
- `api_key`: API authentication key
- `search_limit`: Max search results for automatic injection (default: 5)
- `min_score`: Minimum relevance threshold 0-1 (default: 0.6)
- `memex_cli_path`: Custom path to memex-cli executable

**Note**: After editing settings, restart Claude Code for changes to take effect.

### Global Configuration

Memex-cli uses `~/.memex/config.toml` for global settings:

```toml
[memory]
base_url = "http://localhost:8080"
api_key = "your-api-key-here"
timeout_ms = 30000

[memory.search]
limit = 5
min_score = 0.6
```

## Common Workflows

### Workflow 1: Research with Memory Context

```
1. Ask Claude a question
2. Hook searches memory automatically
3. Claude sees relevant past knowledge in context
4. Claude answers, referencing past experiences
5. New knowledge gets recorded automatically
```

### Workflow 2: Explicit Knowledge Lookup

```
1. Use /test-memory command to verify connectivity
2. Run manual search: memex-cli search --query "..."
3. Review results, identify relevant QA IDs
4. Reference those QA IDs in your next prompt to Claude
```

### Workflow 3: Knowledge Curation

```
1. Review candidate knowledge: memex-cli search --validation-level 0
2. Test promising candidates by referencing them
3. Record hits when candidates prove useful
4. System promotes frequently-hit candidates to verified status
```

## Troubleshooting

For debugging hooks and resolving common issues, use the **hook-troubleshooting** skill or run `/validate-hooks` command.

**Quick checks**:
- Verify memex-cli is in PATH: `memex-cli --version`
- Test service connectivity: `/test-memory`
- Check hook logs: `/view-memory-logs`
- Validate configuration: `/validate-hooks`

## Additional Resources

### Reference Files

For detailed configuration and parameter tuning:
- **`references/search-parameters.md`** - Search parameter optimization guide
- **`references/recording-strategies.md`** - Best practices for knowledge recording

### Example Files

Working examples:
- **`examples/example-queries.md`** - Common search query patterns
- **`examples/settings-template.md`** - Complete settings file template

### Commands

Quick-access commands for common tasks:
- `/setup-memex` - Interactive configuration wizard
- `/test-memory` - Test memex-cli and service connectivity
- `/validate-hooks` - Verify hooks configuration
- `/view-memory-logs` - View recent hook execution logs

## Integration with Development Workflow

### Best Practices

1. **Let automation work**: Trust hooks to capture and inject knowledge automatically
2. **Review periodically**: Check `/view-memory-logs` to see what's being recorded
3. **Curate strategically**: Manually record critical knowledge not captured automatically
4. **Tune parameters**: Adjust `search_limit` and `min_score` based on your needs
5. **Restart after config changes**: Settings require Claude Code restart

### When to Disable

Temporarily disable hooks by setting `enabled: false` in `.claude/memory.local.md`:
- Working on unrelated projects without memory needs
- Debugging other hooks (avoid interference)
- Memory service is unavailable
- Performance optimization needed

Remember to restart Claude Code after changing the enabled flag.

## Summary

The memory plugin provides seamless memory capabilities:
- **Automatic**: Most memory operations happen transparently via hooks
- **Manual**: Direct memex-cli access for explicit control
- **Configurable**: Tune behavior via settings files
- **Debuggable**: Commands and logs for troubleshooting

Start with defaults, observe automatic behavior, then tune settings as needed.
