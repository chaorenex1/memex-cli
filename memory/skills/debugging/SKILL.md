---
name: Debugging
description: Systematic troubleshooting for memory hooks. Use when user asks "debug memory", "hooks not working", "memory errors", or mentions hook issues.
version: 1.0.0
---

# Memory Plugin Debugging

Systematic troubleshooting for memory plugin issues.

## Quick Diagnosis

```
/health      # Run full diagnostics
/logs        # Check for errors
```

## Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| No memory injection | Memory disabled in config | Set `enabled = true` in `[memory]` section |
| "command not found" | memex-cli missing | Install: `cargo build -p memex-cli --release` |
| Connection refused | Service not running | Start memory service |
| Service unreachable | Wrong base_url | Check `base_url` in `[memory]` section |
| Hooks not loading | Config changed, no restart | Reload config or restart |

## Diagnostic Flow

```
1. /health (check all components)
    ↓
2. /logs (view recent errors)
    ↓
3. Identify issue from table above
    ↓
4. Apply fix
    ↓
5. /test (verify)
```

## Issue: Hooks Not Triggering / Memory Not Working

**Check**:
```bash
# Is memory enabled?
grep -A5 "\[memory\]" ~/.memex/config.toml | grep enabled
grep -A5 "\[memory\]" ./config.toml | grep enabled

# Check config validity
python -c "import tomllib; tomllib.load(open('$HOME/.memex/config.toml', 'rb'))"
```

**Fix**:
```bash
# Edit ~/.memex/config.toml or ./config.toml
[memory]
enabled = true

# Reload config or restart service
```

## Issue: memex-cli Not Found

**Check**:
```bash
memex-cli --version
```

**Fix**:
```bash
# Build memex-cli
cd /path/to/memex_cli
cargo build --release -p memex-cli

# Add to PATH
export PATH="$PATH:$PWD/target/release"

# Or add to shell rc (~/.bashrc, ~/.zshrc)
echo 'export PATH="$PATH:/path/to/memex_cli/target/release"' >> ~/.bashrc
```

## Issue: Service Connection Failed

**Check**:
```bash
# Extract base_url from config
grep "base_url" ~/.memex/config.toml ./config.toml 2>/dev/null

# Test connectivity
BASE_URL="http://localhost:8080"  # use actual base_url from config
curl -f "${BASE_URL}/health"
```

**Fix**:
```bash
# 1. Start memory service
# 2. Verify base_url in [memory] section of config.toml
# 3. Check firewall/network settings
```

## Issue: Python Dependencies (for hook scripts)

**Check**:
```bash
python -c "import requests"
```

**Fix**:
```bash
pip install requests toml
```

## Log Analysis

**Log locations**:
```
./run.events.jsonl              # Tool events (JSONL format)
~/.memex/logs/                  # Additional logs (if configured)
```

**Error patterns**:
```
[ERROR] Failed to connect        → Service not running / wrong base_url
[ERROR] Command not found        → memex-cli missing from PATH
[ERROR] Config parse error       → Invalid TOML syntax
```

## Manual Hook Testing

```bash
# Test memory-inject manually
echo '{"user_prompt":"test","session_id":"test-123"}' | \
  python memory/scripts/memory-inject.py

# Should exit with code 0
```

## Recovery

### Reset to Safe State

```bash
# Use /reset command
# Options: Logs only, Config removal, Complete reset
```

### Check Configuration

```bash
# Validate TOML syntax
python -c "import tomllib; tomllib.load(open('$HOME/.memex/config.toml', 'rb'))"

# View memory settings
grep -A10 "\[memory\]" ~/.memex/config.toml
```

## Prevention

1. **Restart after config changes** - Hooks load at session start
2. **Test before relying** - Use `/test` to verify
3. **Monitor logs** - Use `/logs` weekly
4. **Update deps** - Keep Python packages current

## Quick Commands

```
/health    - Full diagnostics
/test      - Quick connectivity check
/logs      - View recent errors
/setup     - Reconfigure
/reset     - Clean reset
```

## Still Having Issues?

Collect diagnostic info:
1. `/health` output
2. `/logs` output
3. System: `python --version`, `memex-cli --version`
4. Config: `cat ~/.memex/config.toml` or `cat ./config.toml`
