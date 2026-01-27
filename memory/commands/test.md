---
name: test
description: Quick connectivity test for memory service
allowed-tools: ["Bash", "Read"]
---

# Test Memory Service

Quick test of memory system connectivity.

## Test Sequence

### Test 1: memex-cli Availability

```bash
if command -v memex-cli &> /dev/null; then
  echo "✅ memex-cli found in PATH"
  memex-cli --version
else
  echo "❌ memex-cli not found in PATH"
  echo "   Install: cd /path/to/memex_cli && cargo build --release -p memex-cli"
fi
```

### Test 2: Configuration

```bash
echo "=== Configuration ==="

# Global config
if [ -f ~/.memex/config.toml ]; then
  echo "✅ Global config: ~/.memex/config.toml"
  grep -A5 "\[memory\]" ~/.memex/config.toml 2>/dev/null || true
else
  echo "⚠️  No global config"
fi

# Project config
if [ -f ./config.toml ]; then
  echo "✅ Project config: ./config.toml"
  grep -A5 "\[memory\]" ./config.toml 2>/dev/null || true
else
  echo "⚠️  No project config - using defaults"
fi
```

### Test 3: Config Validation

```bash
# Check if config is valid TOML
if [ -f ~/.memex/config.toml ]; then
  if command -v tomli &> /dev/null; then
    tomli ~/.memex/config.toml && echo "✅ Config is valid TOML"
  elif command -v python &> /dev/null; then
    python -c "import tomllib; tomllib.load(open('$HOME/.memex/config.toml', 'rb'))" 2>/dev/null && echo "✅ Config is valid TOML"
  fi
fi
```

### Test 4: Service Health

```bash
# Extract base_url from config or use default
BASE_URL="http://localhost:8080"

if [ -f ./config.toml ]; then
  BASE_URL=$(grep "base_url" ./config.toml 2>/dev/null | head -1 | cut -d= -f2 | tr -d '"')
fi

if curl -f -s -o /dev/null -w "%{http_code}" "${BASE_URL}/health" 2>/dev/null | grep -q "200"; then
  echo "✅ Memory service responding at $BASE_URL"
else
  echo "❌ Memory service not reachable at $BASE_URL"
  echo "   → Start service or check [memory] base_url in config.toml"
fi
```

### Test 5: Search Function

```bash
memex-cli search \
  --query "test" \
  --limit 1 \
  --min-score 0.1 \
  --format json 2>&1 | head -10
```

### Test 6: Record Function

```bash
memex-cli record-candidate \
  --query "Test from /test command" \
  --answer "Testing recording functionality" \
  --tags "test,$(date +%Y%m%d)" 2>&1
```

## Results Summary

```
=== Test Results ===

memex-cli:       ✅/❌
Config:          ✅/⚠️
Service:         ✅/❌
Search:          ✅/❌
Record:          ✅/❌

Overall: <PASS/FAIL>
```

## Next Steps

- **PASS**: Memory system ready. Use `/search` to query, `/record` to save knowledge.
- **FAIL**: Run `/health` for detailed diagnostics, or `/logs` to check errors.
