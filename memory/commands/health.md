---
name: health
description: Comprehensive health check and diagnostics
allowed-tools: ["Bash", "Read"]
---

# Health Check

Comprehensive diagnostics for memex-cli memory system.

## Checks

### 1. memex-cli Binary

```bash
echo "=== memex-cli Binary ==="

if command -v memex-cli &> /dev/null; then
  echo "✅ memex-cli in PATH"
  memex-cli --version
else
  echo "❌ memex-cli not found"
  echo "   Install: cd /path/to/memex_cli && cargo build --release -p memex-cli"
fi
```

### 2. Configuration Files

```bash
echo "=== Configuration Files ==="

# Check global config
if [ -f ~/.memex/config.toml ]; then
  echo "✅ Global config exists: ~/.memex/config.toml"
  # Validate TOML syntax
  if command -v python &> /dev/null; then
    python -c "import tomllib; tomllib.load(open('$HOME/.memex/config.toml', 'rb'))" 2>/dev/null && echo "   ✅ Valid TOML"
  fi
else
  echo "⚠️  No global config (using defaults)"
fi

# Check project config
if [ -f ./config.toml ]; then
  echo "✅ Project config exists: ./config.toml"
  if command -v python &> /dev/null; then
    python -c "import tomllib; tomllib.load(open('config.toml', 'rb'))" 2>/dev/null && echo "   ✅ Valid TOML"
  fi
else
  echo "⚠️  No project config"
fi
```

### 3. Memory Section Status

```bash
echo "=== Memory Configuration ==="

# Extract memory settings
if [ -f ~/.memex/config.toml ]; then
  echo "Provider: $(grep -E '^provider' ~/.memex/config.toml | grep -A10 '\[memory\]' | head -1 | cut -d= -f2 | tr -d '"' || echo 'service')"
  echo "Enabled: $(grep -E '^enabled' ~/.memex/config.toml | grep -A10 '\[memory\]' | head -1 | cut -d= -f2 | tr -d '"' || echo 'true')"
  echo "Base URL: $(grep -E '^base_url' ~/.memex/config.toml | grep -A10 '\[memory\]' | head -1 | cut -d= -f2 | tr -d '"' || echo 'default')"
  echo "Search limit: $(grep -E '^search_limit' ~/.memex/config.toml | grep -A10 '\[memory\]' | head -1 | cut -d= -f2 || echo '6')"
  echo "Min score: $(grep -E '^min_score' ~/.memex/config.toml | grep -A10 '\[memory\]' | head -1 | cut -d= -f2 || echo '0.2')"
fi
```

### 4. Service Connectivity

```bash
echo "=== Service Connectivity ==="

BASE_URL=$(grep -E '^base_url' ~/.memex/config.toml ./config.toml 2>/dev/null | head -1 | cut -d= -f2 | tr -d '"')
BASE_URL=${BASE_URL:-"http://localhost:8080"}

if curl -f -s "${BASE_URL}/health" > /dev/null 2>&1; then
  echo "✅ Service reachable at $BASE_URL"
else
  echo "❌ Service not reachable at $BASE_URL"
  echo "   → Check [memory] base_url in config.toml"
fi
```

### 5. Log Files

```bash
echo "=== Log Files ==="

# Check events_out log
if [ -f ./run.events.jsonl ]; then
  lines=$(wc -l < ./run.events.jsonl 2>/dev/null || echo "0")
  size=$(du -h ./run.events.jsonl 2>/dev/null | cut -f1 || echo "unknown")
  echo "✅ run.events.jsonl: $lines lines, $size"
else
  echo "⚠️  No run.events.jsonl (events_out may be disabled)"
fi

# Check log directory
if [ -d ~/.memex/logs ]; then
  echo "✅ Log directory exists: ~/.memex/logs/"
  ls -lh ~/.memex/logs/ 2>/dev/null | tail -5 || true
fi
```

### 6. Events Out Config

```bash
echo "=== Events Out Config ==="

if grep -q '\[events_out\]' ~/.memex/config.toml ./config.toml 2>/dev/null; then
  events_enabled=$(grep -E '^enabled' ~/.memex/config.toml ./config.toml 2>/dev/null | grep -A3 '\[events_out\]' | head -1 | cut -d= -f2 | tr -d '"' || echo "true")
  events_path=$(grep -E '^path' ~/.memex/config.toml ./config.toml 2>/dev/null | grep -A3 '\[events_out\]' | head -1 | cut -d= -f2 | tr -d '"' || echo "./run.events.jsonl")
  echo "Events out: $events_enabled"
  echo "Events path: $events_path"
else
  echo "Using defaults: enabled=true, path=./run.events.jsonl"
fi
```

## Summary

```
=== Health Check Summary ===

Component        Status    Action Needed
---------------  -------   -----------------------
memex-cli        ✅/❌
Config files     ✅/⚠️
Memory config    ✅/⚠️
Service          ✅/❌
Logs             ✅/⚠️
Events out       ✅/⚠️

Overall: <HEALTHY/DEGRADED/UNHEALTHY>
```

## Actions

| Status | Action |
|--------|--------|
| **HEALTHY** | All systems go |
| **DEGRADED** | Non-critical issues - check `/logs` |
| **UNHEALTHY** | Critical issues - see errors above |
