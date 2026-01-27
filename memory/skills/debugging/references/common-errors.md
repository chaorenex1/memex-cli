# Common Hook Errors Catalog

## Error Category: Command Execution

### Error: "python: command not found"

**Full error**:
```
/bin/sh: python: command not found
```

**Cause**: System doesn't have `python` command, only `python3`

**Solution**:
```bash
# Option 1: Create symlink (Linux/macOS)
sudo ln -s /usr/bin/python3 /usr/bin/python

# Option 2: Update hooks.json to use python3
# Edit hooks/hooks.json, replace "python" with "python3":
"command": "python3 ${CLAUDE_PLUGIN_ROOT}/scripts/memory-inject.py"

# Option 3: Use absolute path
which python3  # Get path, e.g., /usr/bin/python3
# Update hooks.json with: "/usr/bin/python3 ${CLAUDE_PLUGIN_ROOT}/..."
```

### Error: "memex-cli: command not found"

**Full error**:
```
/bin/sh: memex-cli: command not found
```

**Cause**: memex-cli not in PATH or not installed

**Solution**:
```bash
# Check installation
ls -l /path/to/memex_cli/target/release/memex-cli

# Option 1: Add to PATH
export PATH="$PATH:/path/to/memex_cli/target/release"
# Make permanent: add to ~/.bashrc or ~/.zshrc

# Option 2: Use settings file
cat > .claude/memory.local.md <<'EOF'
---
enabled: true
memex_cli_path: "/full/path/to/memex_cli/target/release/memex-cli"
---
EOF

# Then update Python scripts to read this setting
```

### Error: "Permission denied"

**Full error**:
```
/bin/sh: ./scripts/memory-inject.py: Permission denied
```

**Cause**: Script not executable (Linux/macOS only)

**Solution**:
```bash
# Make all scripts executable
chmod +x memory/scripts/*.py

# Verify
ls -l memory/scripts/*.py
# Should show: -rwxr-xr-x
```

## Error Category: Python Imports

### Error: "ModuleNotFoundError: No module named 'requests'"

**Full error**:
```
Traceback (most recent call last):
  File "scripts/memory-inject.py", line 3, in <module>
    import requests
ModuleNotFoundError: No module named 'requests'
```

**Cause**: Python requests library not installed

**Solution**:
```bash
# Install from requirements file
pip install -r memory/requirements-http.txt

# Or install directly
pip install requests

# Verify
python -c "import requests; print(requests.__version__)"
```

### Error: "No module named 'yaml'"

**Full error**:
```
ModuleNotFoundError: No module named 'yaml'
```

**Cause**: PyYAML not installed (if scripts use YAML parsing)

**Solution**:
```bash
pip install pyyaml

# Verify
python -c "import yaml; print(yaml.__version__)"
```

## Error Category: Network/Service

### Error: "Connection refused"

**Full error**:
```
requests.exceptions.ConnectionError: HTTPConnectionPool(host='localhost', port=8080):
Max retries exceeded with url: /api/search (Caused by NewConnectionError:
[Errno 61] Connection refused)
```

**Cause**: Memory service not running or wrong port

**Solution**:
```bash
# Check if service is running
curl http://localhost:8080/health

# Check correct port in config
cat ~/.memex/config.toml | grep base_url

# Start service if needed (example)
# cd /path/to/memory-service
# ./start-service.sh

# Or use different port in settings
cat > .claude/memory.local.md <<'EOF'
---
enabled: true
base_url: "http://localhost:9000"  # Use correct port
---
EOF
```

### Error: "Timeout"

**Full error**:
```
requests.exceptions.Timeout: HTTPConnectionPool(host='localhost', port=8080):
Read timed out. (read timeout=30)
```

**Cause**: Service responding too slowly

**Solution**:
```bash
# Option 1: Increase timeout in config
cat >> ~/.memex/config.toml <<'EOF'

[memory]
timeout_ms = 60000  # Increase to 60 seconds
EOF

# Option 2: Optimize service performance
# - Check service logs for slow queries
# - Add database indexes
# - Increase service resources

# Option 3: Reduce search limit
cat > .claude/memory.local.md <<'EOF'
---
search_limit: 3  # Fewer results = faster
---
EOF
```

### Error: "401 Unauthorized"

**Full error**:
```
requests.exceptions.HTTPError: 401 Client Error: Unauthorized for url:
http://localhost:8080/api/search
```

**Cause**: API key missing or incorrect

**Solution**:
```bash
# Check API key in config
cat ~/.memex/config.toml | grep api_key

# Update API key
cat >> ~/.memex/config.toml <<'EOF'

[memory]
api_key = "your-correct-api-key-here"
EOF

# Or use project-specific key
cat > .claude/memory.local.md <<'EOF'
---
api_key: "project-specific-key"
---
EOF
```

## Error Category: JSON/Data

### Error: "JSONDecodeError"

**Full error**:
```
json.decoder.JSONDecodeError: Expecting value: line 1 column 1 (char 0)
```

**Cause**: Invalid JSON response from service or invalid input

**Solution**:
```bash
# Test service response
curl http://localhost:8080/api/search \
  -H "Content-Type: application/json" \
  -d '{"query":"test","limit":1}'

# Should return valid JSON, not HTML error page

# Check service logs for errors
# Fix service issue, then retry
```

### Error: "KeyError: 'user_prompt'"

**Full error**:
```
KeyError: 'user_prompt'
```

**Cause**: Hook input missing expected field

**Solution**:
```python
# Update script to handle missing fields
import sys
import json

try:
    input_data = json.load(sys.stdin)
    user_prompt = input_data.get('user_prompt', '')
    if not user_prompt:
        print('{"continue": true, "suppressOutput": true}')
        sys.exit(0)
except Exception as e:
    print(f'{{"continue": true, "systemMessage": "Hook error: {e}"}}')
    sys.exit(0)
```

## Error Category: Configuration

### Error: "hooks.json: unexpected token"

**Full error**:
```
Error loading hooks: SyntaxError: Unexpected token } in JSON at position 145
```

**Cause**: Invalid JSON syntax in hooks.json

**Solution**:
```bash
# Validate JSON
python -m json.tool memory/hooks/hooks.json

# Common issues:
# - Trailing commas
# - Unquoted keys
# - Missing brackets

# Fix and validate again
```

### Error: "Frontmatter parse error"

**Full error**:
```
Error: Could not parse YAML frontmatter in .claude/memory.local.md
```

**Cause**: Invalid YAML in settings file

**Solution**:
```bash
# Check YAML syntax
cat .claude/memory.local.md | \
  awk '/^---$/{i++; next} i==1' | \
  python -c "import yaml,sys; yaml.safe_load(sys.stdin)"

# Common issues:
# - Tabs instead of spaces (use spaces only)
# - Unquoted values with special chars
# - Missing colon after key

# Example fix:
cat > .claude/memory.local.md <<'EOF'
---
enabled: true
base_url: "http://localhost:8080"
search_limit: 5
---
EOF
```

## Error Category: File System

### Error: "FileNotFoundError: [Errno 2] No such file or directory"

**Full error**:
```
FileNotFoundError: [Errno 2] No such file or directory: '/path/to/transcript.txt'
```

**Cause**: Script trying to access non-existent file

**Solution**:
```python
# Update script to handle missing files
import os

transcript_path = input_data.get('transcript_path')
if transcript_path and os.path.exists(transcript_path):
    with open(transcript_path) as f:
        content = f.read()
else:
    # Gracefully handle missing file
    print('{"continue": true, "suppressOutput": true}')
    sys.exit(0)
```

### Error: "OSError: [Errno 28] No space left on device"

**Full error**:
```
OSError: [Errno 28] No space left on device
```

**Cause**: Disk full, often from log files

**Solution**:
```bash
# Check disk usage
df -h

# Find large log files
du -sh memory/scripts/*.log

# Archive and clean logs
mkdir -p logs-archive
mv memory/scripts/*.log logs-archive/
gzip logs-archive/*.log

# Set up log rotation (create logrotate config)
```

## Error Category: Encoding

### Error: "UnicodeDecodeError"

**Full error**:
```
UnicodeDecodeError: 'utf-8' codec can't decode byte 0xff in position 123
```

**Cause**: Non-UTF-8 file being read

**Solution**:
```python
# Read with error handling
try:
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
except UnicodeDecodeError:
    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
        content = f.read()
```

## Error Category: Hook Lifecycle

### Error: "Hook timeout"

**Full error**:
```
Hook exceeded timeout of 300ms
```

**Cause**: Hook script took too long to execute

**Solution**:
```json
// Increase timeout in hooks.json
{
  "type": "command",
  "command": "python ${CLAUDE_PLUGIN_ROOT}/scripts/memory-inject.py",
  "timeout": 600  // Increase from 300 to 600
}
```

### Error: "Hook blocked by user"

**Full error**:
```
Hook execution blocked: user denied permission
```

**Cause**: User declined hook execution prompt

**Solution**:
```bash
# Add to permissions in .claude/settings.json
{
  "permissions": {
    "allow": [
      "Bash(python memory/scripts/*)"
    ]
  }
}

# Or disable hook temporarily
cat > .claude/memory.local.md <<'EOF'
---
enabled: false
---
EOF
```

## Quick Reference

| Error Pattern | Likely Cause | Quick Fix |
|---------------|--------------|-----------|
| `command not found` | Missing executable | Add to PATH or use absolute path |
| `ModuleNotFoundError` | Missing Python package | `pip install <package>` |
| `Connection refused` | Service not running | Start service, check port |
| `Permission denied` | File not executable | `chmod +x script.py` |
| `JSONDecodeError` | Invalid JSON | Validate with `json.tool` |
| `Timeout` | Slow operation | Increase timeout, optimize |
| `KeyError` | Missing data field | Add null checks in script |
| `FileNotFoundError` | Missing file | Check path, add existence check |

## Debugging Workflow

1. **Identify error category** (command, network, config, etc.)
2. **Check logs** for full error message and stack trace
3. **Apply category-specific solution** from this guide
4. **Test fix** with manual hook execution
5. **Restart Claude Code** to reload hooks
6. **Verify** with `/test-memory` and `/validate-hooks`

For errors not covered here, check `log-format.md` for log analysis techniques.
