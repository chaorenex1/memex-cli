---
name: test-memory
description: Test memex-cli availability and memory service connectivity
allowed-tools: ["Bash", "Read"]
---

# Test Memory System

Comprehensive test of memory plugin components.

## Test Sequence

### Test 1: memex-cli Availability

Check if memex-cli is accessible:

```bash
# Test 1.1: Check in PATH
if command -v memex-cli &> /dev/null; then
  echo "✅ memex-cli found in PATH"
  memex-cli --version
else
  echo "❌ memex-cli not found in PATH"
fi
```

**Expected result**: Version info displayed (e.g., "memex-cli 0.1.0")

**If failed**:
- Read `.claude/memory.local.md` to check custom path
- Try custom path if specified
- Otherwise, inform user memex-cli needs to be installed

### Test 2: Configuration Check

Read and validate configuration:

```bash
# Check global config
if [ -f ~/.memex/config.toml ]; then
  echo "✅ Global config found: ~/.memex/config.toml"
  cat ~/.memex/config.toml | grep -E "(base_url|api_key|timeout)"
else
  echo "⚠️  No global config at ~/.memex/config.toml"
fi

# Check project-specific config
if [ -f .claude/memory.local.md ]; then
  echo "✅ Project config found: .claude/memory.local.md"
  # Parse and display frontmatter
else
  echo "⚠️  No project config - hooks using defaults"
fi
```

### Test 3: Memory Service Health

Test HTTP connectivity to memory service:

```bash
# Extract base_url from config
BASE_URL="http://localhost:8080"  # default

# Try health endpoint
if curl -f -s -o /dev/null -w "%{http_code}" "${BASE_URL}/health"; then
  echo "✅ Memory service responding at ${BASE_URL}"
  curl -s "${BASE_URL}/health" | head -20
else
  echo "❌ Memory service not reachable at ${BASE_URL}"
  echo "   Start service or check base_url in config"
fi
```

**Expected result**: HTTP 200 and health check response

### Test 4: Search Functionality

Execute test search query:

```bash
# Test search with minimal parameters
memex-cli search \
  --query "test connection" \
  --limit 1 \
  --min-score 0.1 \
  --format json

echo "Exit code: $?"
```

**Expected results**:
- Exit code: 0 (success)
- JSON response with results array (may be empty if no matching content)
- No connection errors

**If failed**:
- Check error message
- Common issues:
  - Connection refused → service not running
  - 401 Unauthorized → API key incorrect
  - Timeout → service overloaded

### Test 5: Record Functionality

Test recording a candidate knowledge entry:

```bash
# Record test entry
memex-cli record-candidate \
  --query "Test entry from /test-memory" \
  --answer "This is a test to verify recording functionality" \
  --tags "test,setup,$(date +%Y%m%d)"

echo "Exit code: $?"
```

**Expected result**:
- Exit code: 0
- Confirmation message with qa_id

**If failed**:
- Check if write permissions exist
- Verify service accepts POST requests

### Test 6: Search for Test Entry

Verify recorded entry is searchable:

```bash
# Wait briefly for indexing
sleep 2

# Search for just-recorded entry
memex-cli search \
  --query "Test entry from test-memory" \
  --limit 3 \
  --min-score 0.5 \
  --format markdown
```

**Expected result**: Should find the test entry in results

### Test 7: Python Dependencies

Check if hook scripts can run:

```bash
# Test Python and imports
python -c "import sys, json, subprocess; print('✅ Python imports OK')" 2>&1

# Check requests library (needed for HTTP client)
python -c "import requests; print('✅ requests library OK')" 2>&1

# If requests missing
if ! python -c "import requests" 2>/dev/null; then
  echo "❌ requests library not found"
  echo "   Install with: pip install -r memory/requirements-http.txt"
fi
```

### Test 8: Hook Scripts Validation

Check if hook scripts exist and are valid:

```bash
# Check script files exist
for script in session-init.py memory-inject.py memory-record.py \
              record-session-enhanced.py session-cleanup.py; do
  if [ -f "memory/scripts/$script" ]; then
    echo "✅ $script exists"
    # Check syntax
    python -m py_compile "memory/scripts/$script" 2>&1
  else
    echo "❌ $script missing"
  fi
done
```

## Results Summary

After all tests, provide summary:

```
=== Test Results Summary ===

Component              Status    Details
--------------------   -------   -----------------------
memex-cli              ✅/❌     <version or error>
Global config          ✅/⚠️     <found or not found>
Project config         ✅/⚠️     <found or not found>
Memory service         ✅/❌     <reachable or error>
Search function        ✅/❌     <working or error>
Record function        ✅/❌     <working or error>
Python dependencies    ✅/❌     <ok or missing packages>
Hook scripts           ✅/❌     <valid or issues>

Overall: <PASS/PARTIAL/FAIL>
```

**Interpretation**:
- **PASS**: All core components working (memex-cli, service, search, record)
- **PARTIAL**: Some issues but basic functionality works
- **FAIL**: Critical components not working

## Recommendations

Based on test results, provide actionable recommendations:

**If memex-cli missing**:
```
Install memex-cli:
cd /path/to/memex_cli
cargo build --release -p memex-cli
export PATH="$PATH:$(pwd)/target/release"
```

**If service not reachable**:
```
1. Start memory service (if implemented)
2. Or check base_url in configuration
3. Or verify firewall/network settings
```

**If dependencies missing**:
```
pip install -r memory/requirements-http.txt
```

**If all tests pass**:
```
✅ Memory system is fully operational!

Try automatic features:
1. Ask Claude a question
2. Hook will search memory and inject relevant context
3. Check /view-memory-logs to see injection in action

Or use manual commands:
memex-cli search --query "your question" --limit 5
```

## Cleanup

After testing, optionally clean up test entry:

```
Note: Test entry "Test entry from /test-memory" was created.
You can remove it manually if desired, or leave it for future reference.
```

## Troubleshooting

If any test fails, refer to the hook-troubleshooting skill:
- Use `/validate-hooks` for detailed hook validation
- Use `/view-memory-logs` to check recent errors
- Ask: "troubleshoot memory hooks" to load debugging skill
