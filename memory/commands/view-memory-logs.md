---
name: view-memory-logs
description: View recent hook execution logs for debugging
allowed-tools: ["Bash", "Read"]
---

# View Memory Hook Logs

Display recent logs from memory hook executions.

## Log Files

Hook scripts write to these log files:
- `memory/scripts/memory-inject.log` - UserPromptSubmit hook (memory search/injection)
- `memory/scripts/memory-record.log` - PostToolUse hook (knowledge recording)
- `memory/scripts/session-init.log` - SessionStart hook
- `memory/scripts/session-cleanup.log` - SessionEnd hook
- `memory/scripts/record-session.log` - Stop/SubagentStop/PreCompact hooks

## Display Recent Logs

Show last 50 lines from each log file:

```bash
echo "=== Memory Hook Logs ==="
echo

for log_file in memory/scripts/*.log; do
  if [ -f "$log_file" ]; then
    log_name=$(basename "$log_file")
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📄 $log_name"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo

    # Show file size
    size=$(ls -lh "$log_file" | awk '{print $5}')
    echo "Size: $size"
    echo

    # Show last 50 lines
    tail -50 "$log_file"
    echo
    echo
  fi
done
```

## Filter by Error Level

Show only errors and warnings:

```bash
echo "=== Errors and Warnings ==="
echo

for log_file in memory/scripts/*.log; do
  if [ -f "$log_file" ]; then
    log_name=$(basename "$log_file")

    # Extract ERROR and WARNING lines
    errors=$(grep -E "\[ERROR\]|\[WARNING\]" "$log_file" 2>/dev/null | tail -20)

    if [ -n "$errors" ]; then
      echo "━━━ $log_name ━━━"
      echo "$errors"
      echo
    fi
  fi
done

# If no errors found
if ! grep -r -E "\[ERROR\]|\[WARNING\]" memory/scripts/*.log 2>/dev/null; then
  echo "✅ No errors or warnings found in recent logs"
fi
```

## Recent Activity Summary

Show activity from last 24 hours:

```bash
echo "=== Recent Activity (Last 24 Hours) ==="
echo

# Get date 24 hours ago (platform-specific)
if [[ "$OSTYPE" == "darwin"* ]]; then
  # macOS
  yesterday=$(date -v-24H '+%Y-%m-%d')
else
  # Linux
  yesterday=$(date -d '24 hours ago' '+%Y-%m-%d')
fi

for log_file in memory/scripts/*.log; do
  if [ -f "$log_file" ]; then
    log_name=$(basename "$log_file" .log)

    # Count entries from today and yesterday
    count=$(grep -cE "$yesterday|$(date '+%Y-%m-%d')" "$log_file" 2>/dev/null || echo "0")

    if [ "$count" -gt 0 ]; then
      echo "✅ $log_name: $count entries"
    else
      echo "⚠️  $log_name: No recent activity"
    fi
  fi
done
```

## Log Statistics

```bash
echo -e "\n=== Log Statistics ==="
echo

for log_file in memory/scripts/*.log; do
  if [ -f "$log_file" ]; then
    log_name=$(basename "$log_file" .log)

    # Total lines
    total=$(wc -l < "$log_file")

    # Error count
    errors=$(grep -c "\[ERROR\]" "$log_file" 2>/dev/null || echo "0")

    # Warning count
    warnings=$(grep -c "\[WARNING\]" "$log_file" 2>/dev/null || echo "0")

    # Success count
    success=$(grep -c -E "success|completed|injected" "$log_file" 2>/dev/null || echo "0")

    printf "%-30s Total: %5d | Errors: %3d | Warnings: %3d | Success: %5d\n" \
      "$log_name" "$total" "$errors" "$warnings" "$success"
  fi
done
```

## Tail Specific Log

To follow a specific log in real-time (for debugging):

```bash
echo -e "\n=== Available Commands for Real-Time Monitoring ==="
echo
echo "To follow memory-inject.log:"
echo "  tail -f memory/scripts/memory-inject.log"
echo
echo "To follow memory-record.log:"
echo "  tail -f memory/scripts/memory-record.log"
echo
echo "To follow all logs:"
echo "  tail -f memory/scripts/*.log"
echo
echo "Press Ctrl+C to stop following"
```

## Search Logs

Search for specific patterns:

```bash
read -p "Enter search term (or press Enter to skip): " search_term

if [ -n "$search_term" ]; then
  echo -e "\n=== Search Results for: $search_term ==="
  echo

  for log_file in memory/scripts/*.log; do
    if [ -f "$log_file" ]; then
      matches=$(grep -i "$search_term" "$log_file" 2>/dev/null)

      if [ -n "$matches" ]; then
        log_name=$(basename "$log_file")
        echo "━━━ $log_name ━━━"
        echo "$matches" | tail -20
        echo
      fi
    fi
  done
fi
```

## Log Maintenance

Show log cleanup options:

```bash
echo -e "\n=== Log Maintenance ==="
echo

# Calculate total log size
total_size=$(du -sh memory/scripts/*.log 2>/dev/null | awk '{sum+=$1} END {print sum}')

echo "Current log files:"
ls -lh memory/scripts/*.log 2>/dev/null || echo "No log files found"

echo -e "\nLog cleanup options:"
echo "  1. Archive old logs:"
echo "     mkdir -p memory/logs-archive"
echo "     mv memory/scripts/*.log memory/logs-archive/"
echo "     gzip memory/logs-archive/*.log"
echo
echo "  2. Clear all logs:"
echo "     rm memory/scripts/*.log"
echo
echo "  3. Keep only recent logs (last 100 lines):"
echo "     for log in memory/scripts/*.log; do"
echo "       tail -100 \$log > \$log.tmp && mv \$log.tmp \$log"
echo "     done"
```

## Interpretation Guide

**Log entry patterns**:

```
[2026-01-12 16:00:00] [INFO] <message>     - Normal operation
[2026-01-12 16:00:01] [WARNING] <message>  - Non-critical issue
[2026-01-12 16:00:02] [ERROR] <message>    - Failed operation
```

**Common log entries**:

**memory-inject.log**:
- `User prompt: <query>` - User submitted a prompt
- `Search results: N items` - Memory search found N results
- `Context injected successfully` - Memory added to conversation
- `No results found` - No relevant memory for this query

**memory-record.log**:
- `Tool used: Write|Edit|Bash` - Tool execution detected
- `Recording candidate` - Saving knowledge to memory
- `Query: <extracted query>` - What problem was solved
- `Answer: <extracted answer>` - How it was solved

**Errors to watch for**:
- `Connection refused` - Memory service not running
- `command not found: memex-cli` - memex-cli not in PATH
- `ModuleNotFoundError` - Python dependencies missing
- `Timeout` - Operation took too long

## Quick Actions

Based on log analysis, suggest actions:

```bash
echo -e "\n=== Recommended Actions ==="

# Check for common issues
if grep -q "Connection refused" memory/scripts/*.log 2>/dev/null; then
  echo "⚠️  Connection errors detected"
  echo "   → Start memory service or check base_url in config"
fi

if grep -q "command not found: memex-cli" memory/scripts/*.log 2>/dev/null; then
  echo "⚠️  memex-cli not found"
  echo "   → Run /setup-memex to configure path"
fi

if grep -q "ModuleNotFoundError" memory/scripts/*.log 2>/dev/null; then
  echo "⚠️  Python dependencies missing"
  echo "   → pip install -r memory/requirements-http.txt"
fi

if ! grep -q "\[ERROR\]" memory/scripts/*.log 2>/dev/null; then
  echo "✅ No errors found - hooks working normally"
fi
```

## Output Format

Present logs in chronological order with clear section separators. Use:
- 📄 for log file headers
- ✅ for successful operations
- ⚠️ for warnings
- ❌ for errors

Provide context and recommendations based on log content.
