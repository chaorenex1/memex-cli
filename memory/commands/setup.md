---
name: setup
description: Interactive configuration wizard for memex-cli
allowed-tools: ["AskUserQuestion", "Write", "Read", "Bash"]
---

# Setup Memex-CLI Configuration

Interactive wizard to configure memex-cli memory settings.

## Config File Locations

**Priority order** (highest first):
1. `~/.memex/config.toml` - Global user configuration
2. `./config.toml` - Project-specific configuration
3. Built-in defaults (if no config exists)

## Steps

### 1. Check Existing Configuration

```bash
# Check global config
if [ -f ~/.memex/config.toml ]; then
  echo "Found global config: ~/.memex/config.toml"
  cat ~/.memex/config.toml | grep -A10 "\[memory\]"
fi

# Check project config
if [ -f ./config.toml ]; then
  echo "Found project config: ./config.toml"
  cat ./config.toml | grep -A10 "\[memory\]"
fi
```

### 2. Gather Settings

Ask the following questions:

**Q1: Memory Provider**
- "What memory provider to use?"
  - "service (Remote HTTP API - recommended)"
  - "local (LanceDB - experimental)"
  - "hybrid (Local + Remote sync - experimental)"

**Q2: Memory Service URL** (if provider=service)
- Options:
  - "http://localhost:8080"
  - "http://localhost:9000"
  - "https://memory.internal"
  - "Custom URL"

**Q3: API Key** (if provider=service)
- "Does your service require authentication?"
  - "Yes - I have an API key"
  - "No - Open access"

**Q4: Search Results Limit**
- "How many results to inject automatically? (default: 6)"
  - "3 (minimal)"
  - "6 (balanced - default)"
  - "10 (comprehensive)"
  - "Custom number"

**Q5: Relevance Threshold**
- "Minimum relevance score 0.0-1.0? (default: 0.2)"
  - "0.4 (strict)"
  - "0.2 (balanced - default)"
  - "0.1 (permissive)"

**Q6: Enable Memory**
- "Enable memory features?"
  - "Yes - enabled"
  - "No - disabled"

### 3. Write Configuration

Create or update `~/.memex/config.toml`:

```toml
[memory]
provider = "service"              # service | local | hybrid
enabled = true                     # Enable/disable memory

# Service provider settings
base_url = "<user_url>"            # Memory service URL
api_key = "<user_key>"             # API key (leave empty if none)
timeout_ms = 10000                 # Request timeout (ms)

# Search parameters
search_limit = <user_limit>        # Max results (default: 6)
min_score = <user_score>           # Min relevance 0-1 (default: 0.2)

# Local provider settings (if provider=local)
# db_path = "~/.memex/db"
#
# [memory.embedding]
# provider = "ollama"              # ollama | openai | local
# model = "nomic-embed-text"
# dimension = 768
```

### 4. Optional: Additional Sections

Add these sections as needed:

```toml
# Control how memory is injected into prompts
[prompt_inject]
placement = "user"                # system | user
max_items = 10                    # Max items to inject
max_answer_chars = 1000           # Truncate long answers
include_meta_line = true          # Show QA metadata

# Quality gates for memory
[gatekeeper]
provider = "standard"
max_inject = 10                   # Max items to inject
min_trust_show = 0.40             # Min trust score to show
min_level_inject = 2              # Min validation level
min_level_fallback = 1
block_if_consecutive_fail_ge = 3
skip_if_top1_score_ge = 0.85

# Knowledge extraction rules
[candidate_extract]
max_candidates = 10
max_answer_chars = 2000
min_answer_chars = 100
redact = true                     # Redact sensitive info
strict_secret_block = true
confidence = 0.45

# Tool event logging
[events_out]
enabled = true
path = "./run.events.jsonl"       # Output file path
channel_capacity = 2048
drop_when_full = true
```

### 5. Update .gitignore

```bash
# Protect sensitive config
if ! grep -q "config.toml" .gitignore 2>/dev/null; then
  echo "# Local configuration" >> .gitignore
  echo "config.toml" >> .gitignore
fi
```

### 6. Verify Installation

```bash
# Test memex-cli
memex-cli --version

# Test service connectivity (if using service provider)
curl -f -s <base_url>/health || echo "Service not reachable"

# Test memory search
memex-cli search --query "test" --limit 1
```

### 7. Summary

```
Configuration: ~/.memex/config.toml (or ./config.toml)

Settings:
  Provider: <provider>
  Service URL: <base_url>
  Search limit: <limit>
  Min score: <score>
  Enabled: <enabled>

Next steps:
  1. Test: /test
  2. Check health: /health
  3. View logs: /logs
```

## Config Reference

| Section | Key | Default | Description |
|---------|-----|---------|-------------|
| [memory] | provider | service | Memory backend |
| [memory] | enabled | true | Enable memory features |
| [memory] | base_url | https://memory.internal | Service URL |
| [memory] | api_key | "" | Authentication key |
| [memory] | timeout_ms | 10000 | Request timeout |
| [memory] | search_limit | 6 | Max results |
| [memory] | min_score | 0.2 | Min relevance |
| [prompt_inject] | placement | user | Where to inject |
| [prompt_inject] | max_items | 10 | Max items to inject |
| [gatekeeper] | min_level_inject | 2 | Min validation level |
| [events_out] | enabled | true | Log tool events |
| [events_out] | path | ./run.events.jsonl | Output file |

## Provider Options

### Service (Remote HTTP)
- Requires running memory service
- Best for: Team collaboration, cloud storage
- Config: `base_url`, `api_key`, `timeout_ms`

### Local (LanceDB)
- Requires LanceDB installation
- Best for: Offline use, local development
- Config: `db_path`, embedding provider

### Hybrid
- Combines local + remote
- Best for: Offline work with sync
- Config: Both local and remote sections
