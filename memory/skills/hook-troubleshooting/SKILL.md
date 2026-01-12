---
name: Hook Troubleshooting
description: This skill should be used when the user asks to "debug hooks", "hooks not working", "troubleshoot memory hooks", "check hook logs", "fix hook errors", or mentions hook execution problems.
version: 1.0.0
---

# Hook Troubleshooting Guide

Diagnose and resolve issues with memory plugin hooks.

## Quick Diagnostics

### Step 1: Verify Hook Registration

Check if hooks are loaded in current session:

```bash
# In Claude Code, type:
/hooks
```

Look for memory hooks in the output. If not present, hooks didn't load.

### Step 2: Check Hook Logs

View recent hook execution logs:

```bash
# Use plugin command:
/view-memory-logs

# Or manually check logs:
ls -lh memory/scripts/*.log
tail -50 memory/scripts/memory-inject.log
```

### Step 3: Test Components

```bash
# Test memex-cli availability
memex-cli --version

# Test memory service
/test-memory

# Validate hooks configuration
/validate-hooks
```

## Common Issues

### Issue 1: Hooks Not Triggering

**Symptom**: No memory injection, no knowledge recording

**Causes**:
1. **Settings disabled**: `enabled: false` in `.claude/memory.local.md`
2. **Hook configuration not loaded**: Changed `hooks.json` but didn't restart
3. **Invalid hooks.json**: JSON syntax errors
4. **Python not found**: `python` not in PATH

**Solutions**:

```bash
# Check if enabled
cat .claude/memory.local.md | grep "enabled"

# Restart Claude Code (hooks load at session start)
# Exit current session and run: claude

# Validate hooks.json syntax
cat memory/hooks/hooks.json | python -m json.tool

# Check Python availability
which python
python --version
```

### Issue 2: memex-cli Not Found

**Symptom**: Logs show "command not found: memex-cli"

**Causes**:
1. memex-cli not installed
2. Not in PATH
3. Different executable name (memex-cli.exe on Windows)

**Solutions**:

```bash
# Install memex-cli
cd /path/to/memex_cli
cargo build --release -p memex-cli

# Add to PATH (Linux/macOS)
export PATH="$PATH:/path/to/memex_cli/target/release"

# Or use custom path in settings
cat > .claude/memory.local.md <<'EOF'
---
enabled: true
memex_cli_path: "/full/path/to/memex-cli"
---
EOF
```

### Issue 3: Memory Service Connection Failed

**Symptom**: Logs show connection errors, timeouts

**Causes**:
1. Memory service not running
2. Wrong base_url in configuration
3. Network/firewall blocking connection
4. API key mismatch

**Solutions**:

```bash
# Check service status
curl http://localhost:8080/health

# Start service if needed
# (depends on your memory service implementation)

# Verify configuration
cat ~/.memex/config.toml
cat .claude/memory.local.md

# Test with explicit URL
memex-cli search --query "test" --base-url "http://localhost:8080"
```

### Issue 4: Python Dependencies Missing

**Symptom**: Import errors in logs (e.g., "ModuleNotFoundError: No module named 'requests'")

**Causes**:
1. requirements-http.txt not installed
2. Wrong Python environment
3. Virtual environment not activated

**Solutions**:

```bash
# Install dependencies
pip install -r memory/requirements-http.txt

# Or install individually
pip install requests

# Check installed packages
pip list | grep requests
```

### Issue 5: Permission Denied

**Symptom**: "Permission denied" errors for Python scripts (Linux/macOS)

**Cause**: Scripts not executable

**Solution**:

```bash
# Make scripts executable
chmod +x memory/scripts/*.py

# Or run via python explicitly (already done in hooks.json)
# No change needed if using: python ${CLAUDE_PLUGIN_ROOT}/scripts/xxx.py
```

## Log Analysis

### Understanding Log Entries

**Normal memory-inject.log entry**:
```
[2026-01-12 16:00:00] User prompt: How to implement JWT auth?
[2026-01-12 16:00:01] Search results: 3 items (scores: 0.85, 0.72, 0.68)
[2026-01-12 16:00:01] Context injected successfully
```

**Error patterns**:

```
# Connection error
[ERROR] Failed to connect to memory service: Connection refused

# memex-cli not found
[ERROR] Command not found: memex-cli

# Python import error
[ERROR] ModuleNotFoundError: No module named 'requests'

# Configuration error
[ERROR] Missing required setting: base_url
```

### Log Location

```
memory/scripts/
├── memory-inject.log      # UserPromptSubmit hook logs
├── memory-record.log      # PostToolUse hook logs
├── session-init.log       # SessionStart hook logs
└── session-cleanup.log    # SessionEnd hook logs
```

## Debugging Techniques

### Enable Verbose Logging

Edit hook scripts temporarily to add debug output:

```python
# In scripts/memory-inject.py
import sys
import json

# Add after reading input
input_data = json.load(sys.stdin)
print(f"[DEBUG] Received input: {json.dumps(input_data, indent=2)}", file=sys.stderr)
```

### Test Hooks Manually

```bash
# Simulate UserPromptSubmit hook
echo '{"user_prompt":"test query","session_id":"test-123"}' | \
  python memory/scripts/memory-inject.py

# Check exit code
echo "Exit code: $?"

# View output
```

### Check Hook Execution with Debug Mode

```bash
# Start Claude Code in debug mode
claude --debug

# Hooks will show detailed execution info:
# - Hook triggered
# - Input JSON
# - Output JSON
# - Exit code
# - Timing information
```

## Configuration Validation

### Validate hooks.json Structure

```bash
# Check JSON syntax
python -c "import json; json.load(open('memory/hooks/hooks.json'))" && echo "Valid JSON"

# Verify required fields
cat memory/hooks/hooks.json | \
  python -c "import json,sys; d=json.load(sys.stdin); print('hooks' in d.get('hooks',{}))"
```

### Validate Settings File

```bash
# Check YAML frontmatter
cat .claude/memory.local.md | head -20

# Extract and validate YAML
cat .claude/memory.local.md | \
  awk '/^---$/{i++; next} i==1' | \
  python -c "import yaml,sys; yaml.safe_load(sys.stdin)" && echo "Valid YAML"
```

## Recovery Procedures

### Reset to Defaults

```bash
# Disable hooks temporarily
cat > .claude/memory.local.md <<'EOF'
---
enabled: false
---

# Hooks disabled for troubleshooting
EOF

# Restart Claude Code
# Fix issues
# Re-enable: enabled: true
# Restart again
```

### Clean Logs

```bash
# Archive old logs
mkdir -p memory/logs-archive
mv memory/scripts/*.log memory/logs-archive/

# Start fresh
# New .log files will be created on next hook execution
```

### Reinstall Dependencies

```bash
# Uninstall and reinstall
pip uninstall -y requests
pip install -r memory/requirements-http.txt
```

## Advanced Troubleshooting

### Network Debugging

```bash
# Test memory service endpoint
curl -v http://localhost:8080/health

# Test search endpoint
curl -X POST http://localhost:8080/api/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{"query":"test","limit":1}'
```

### Script Debugging

```bash
# Add set -x to hook scripts temporarily
# Edit scripts/memory-inject.py:
#!/usr/bin/env python3
# import sys; sys.stderr.write("DEBUG: Starting script\n")

# Run and capture stderr
echo '{"user_prompt":"test"}' | \
  python memory/scripts/memory-inject.py 2>debug.log

cat debug.log
```

## Getting Help

### Diagnostic Information to Collect

When reporting issues, include:

1. **System info**:
   ```bash
   python --version
   memex-cli --version
   uname -a  # or: ver (Windows)
   ```

2. **Configuration**:
   ```bash
   cat .claude/memory.local.md
   cat ~/.memex/config.toml
   ```

3. **Recent logs**:
   ```bash
   tail -50 memory/scripts/*.log
   ```

4. **Hook status**:
   ```bash
   # In Claude Code: /hooks
   # Copy output
   ```

### Common Solutions Summary

| Problem | Quick Fix |
|---------|-----------|
| Hooks not triggering | Restart Claude Code |
| memex-cli not found | Set `memex_cli_path` in settings |
| Connection failed | Check service running, verify base_url |
| Import errors | `pip install -r requirements-http.txt` |
| Permission denied | `chmod +x scripts/*.py` |
| Invalid JSON | Validate with `python -m json.tool` |

## Reference Files

For detailed patterns and examples:
- **`references/common-errors.md`** - Comprehensive error catalog with solutions
- **`references/log-format.md`** - Log entry format and interpretation guide

## Prevention

### Best Practices

1. **Always restart after config changes**: Hooks load at session start
2. **Test in isolation**: Use `/test-memory` before relying on automatic hooks
3. **Monitor logs regularly**: Check `/view-memory-logs` weekly
4. **Keep dependencies updated**: Periodically update Python packages
5. **Validate before deploying**: Run `/validate-hooks` after changes

### Health Checks

Create a simple health check routine:

```bash
#!/bin/bash
# hooks-health-check.sh

echo "=== Memex-Memory Hooks Health Check ==="

# 1. Check memex-cli
if command -v memex-cli &> /dev/null; then
  echo "✅ memex-cli found: $(memex-cli --version)"
else
  echo "❌ memex-cli not found"
fi

# 2. Check service
if curl -s http://localhost:8080/health > /dev/null; then
  echo "✅ Memory service responding"
else
  echo "❌ Memory service not reachable"
fi

# 3. Check Python dependencies
if python -c "import requests" 2>/dev/null; then
  echo "✅ Python dependencies installed"
else
  echo "❌ Python dependencies missing"
fi

# 4. Check hooks.json
if python -m json.tool memory/hooks/hooks.json > /dev/null 2>&1; then
  echo "✅ hooks.json valid"
else
  echo "❌ hooks.json invalid"
fi

echo "=== Check complete ==="
```

Run before important sessions:
```bash
bash hooks-health-check.sh
```

## Summary

Most hook issues fall into these categories:
1. **Configuration**: Wrong settings or syntax errors → Validate with `/validate-hooks`
2. **Dependencies**: Missing tools or packages → Install with proper commands
3. **Connectivity**: Service unavailable → Test with `/test-memory`
4. **Lifecycle**: Changes not applied → Restart Claude Code

Start with quick diagnostics, check logs, and use plugin commands to identify and fix issues systematically.
