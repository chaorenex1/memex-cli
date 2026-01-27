---
name: Memory Workflow
description: Guide for using memory features effectively. Use when user asks "use memory", "search memory", "record knowledge", "how memory works", or mentions memory functionality.
version: 1.0.0
---

# Memory Workflow Guide

Complete guide to using memory plugin features effectively.

## Overview

The memory plugin provides:
- **Automatic injection**: Searches relevant context when you prompt
- **Automatic recording**: Captures knowledge from tool usage
- **Manual operations**: Direct search and record commands

## Automatic Features

### Memory Injection

**When**: Every time you submit a prompt

**Flow**:
```
You ask a question
    ↓
Hook searches memory service
    ↓
Top results injected into context
    ↓
Claude sees and references past knowledge
```

**Example**:
```
You: "How do I implement JWT auth?"

[Automatic] Searching memory for "JWT auth"...
Found 3 relevant results:
  [qa-12345] JWT best practices (score: 0.85)
  [qa-12346] Token storage options (score: 0.72)

Claude: Based on [qa-12345], I recommend using httpOnly cookies...
```

### Knowledge Recording

**When**: After Write, Edit, or Bash tools execute

**Flow**:
```
Claude uses a tool
    ↓
Hook extracts query (what) and answer (how)
    ↓
Saved as candidate knowledge
    ↓
Validated through usage → promoted to verified
```

**Recorded tools**:
- **Write**: File creation
- **Edit**: File modifications
- **Bash**: Commands (skips simple ones like `ls`)

## Manual Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `/search` | Find past knowledge | `/search` → "JWT authentication" |
| `/record` | Save knowledge | `/record` → Q&A pair |
| `/test` | Check connectivity | `/test` |
| `/health` | Full diagnostics | `/health` |
| `/logs` | View recent logs | `/logs` |
| `/setup` | Configure plugin | `/setup` |

## Common Workflows

### Workflow 1: Research with Context

```
1. Ask your question
2. Memory auto-injects relevant past knowledge
3. Claude references that knowledge
4. New knowledge gets auto-recorded
```

### Workflow 2: Explicit Lookup

```
1. /search → enter query
2. Review results with QA IDs
3. Reference in next prompt: "Based on [qa-12345], how do I..."
```

### Workflow 3: Knowledge Curation

```
1. /record → save important knowledge
2. Future searches retrieve it
3. Using it promotes validation level
```

## Configuration

Edit `~/.memex/config.toml` (global) or `./config.toml` (project):

```toml
[memory]
provider = "service"              # service | local | hybrid
enabled = true                     # Enable/disable memory
base_url = "http://localhost:8080" # Memory service URL
api_key = ""                       # API key (if required)
timeout_ms = 10000                 # Request timeout
search_limit = 6                   # Max results to inject
min_score = 0.2                    # Min relevance (0-1)

[prompt_inject]
placement = "user"                 # system | user
max_items = 10                     # Max items to inject
max_answer_chars = 1000            # Truncate long answers
include_meta_line = true           # Show QA metadata

[gatekeeper]
provider = "standard"
max_inject = 10                    # Max items to inject
min_trust_show = 0.40              # Min trust score to show
min_level_inject = 2               # Min validation level

[candidate_extract]
max_candidates = 10                # Max candidates to extract
max_answer_chars = 2000            # Truncate long answers
min_answer_chars = 100             # Min answer length
redact = true                      # Redact sensitive info
```

**Parameters**:
- `search_limit`: 3 (minimal), 6 (balanced/default), 10 (comprehensive)
- `min_score`: 0.4 (strict), 0.2 (balanced/default), 0.1 (permissive)

## Best Practices

1. **Trust automation** - Let hooks capture knowledge automatically
2. **Review periodically** - Use `/logs` to see what's recorded
3. **Tune parameters** - Adjust based on your needs
4. **Reference QA IDs** - Use `[QA_REF qa-xxxxx]` in prompts
5. **Restart after config** - Settings require Claude Code restart

## When to Disable

Set `enabled = false` in `[memory]` section of config.toml:
- Working on unrelated projects
- Debugging other hooks
- Service unavailable
- Performance critical tasks

Restart memex-cli or re-run after changing configuration.

## Quick Reference

```
/search     - Find past knowledge
/record     - Save new knowledge
/test       - Quick connectivity check
/health     - Full diagnostics
/logs       - View recent logs
/setup      - Configure plugin
/reset      - Clean up/reset state
```

## Troubleshooting

For issues, use:
- `/health` - Diagnose problems
- `/logs` - Check recent errors
- **debugging skill** - Load: "debug memory hooks"
