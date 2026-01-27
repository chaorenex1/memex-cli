---
name: logs
description: View recent execution logs
allowed-tools: ["Bash", "Read"]
---

# View Logs

Display recent logs from memex-cli execution.

## Available Logs

```
./run.events.jsonl        # Tool events (JSONL format)
~/.memex/logs/            # Log directory (if configured)
```

## Display Recent Events

```bash
echo "=== Recent Tool Events (last 20) ==="
echo

if [ -f ./run.events.jsonl ]; then
  # Show file info
  lines=$(wc -l < ./run.events.jsonl 2>/dev/null || echo "0")
  size=$(du -h ./run.events.jsonl 2>/dev/null | cut -f1)
  echo "File: ./run.events.jsonl"
  echo "Size: $size ($lines lines)"
  echo

  # Show last 20 events
  tail -20 ./run.events.jsonl | while read -r line; do
    event_type=$(echo "$line" | jq -r '.event_type // .type' 2>/dev/null || echo "unknown")
    timestamp=$(echo "$line" | jq -r '.timestamp // .ts' 2>/dev/null | cut -dT -f1-2 | cut -d. -f1-2 || echo "")
    echo "[$event_type] $timestamp"
  done
else
  echo "⚠️  No run.events.jsonl found"
  echo "   Check [events_out] section in config.toml"
fi
```

## Filter by Event Type

```bash
echo "=== Filter by Event Type ==="
echo

# Show specific event types
for type in "tool.request" "tool.result" "assistant.output" "event.end"; do
  count=$(jq -r "select(.event_type == \"$type\")" ./run.events.jsonl 2>/dev/null | wc -l || echo "0")
  if [ "$count" -gt 0 ]; then
    echo "$type: $count events"
  fi
done
```

## Errors and Warnings

```bash
echo "=== Errors and Warnings ==="
echo

if [ -f ./run.events.jsonl ]; then
  # Look for error patterns
  errors=$(jq -r 'select(.event_type | test("error"; "fail"; "fatal")) | .event_type' ./run.events.jsonl 2>/dev/null | sort -u | wc -l)

  if [ "$errors" -gt 0 ]; then
    jq -r 'select(.event_type | test("error"; "fail"; "fatal"))' ./run.events.jsonl 2>/dev/null | tail -10
  else
    echo "✅ No error events found in recent logs"
  fi
fi
```

## Statistics

```bash
echo "=== Event Statistics ==="
echo

if [ -f ./run.events.jsonl ]; then
  total=$(wc -l < ./run.events.jsonl)
  echo "Total events: $total"

  echo
  echo "Event types:"
  jq -r '.event_type' ./run.events.jsonl 2>/dev/null | sort | uniq -c | sort -rn | head -10
fi
```

## Log File Info

```bash
echo "=== Log File Info ==="
echo

# Main events file
if [ -f ./run.events.jsonl ]; then
  ls -lh ./run.events.jsonl
  echo

  # First event time
  first_time=$(head -1 ./run.events.jsonl | jq -r '.timestamp // .ts' 2>/dev/null | cut -c1-19)
  # Last event time
  last_time=$(tail -1 ./run.events.jsonl | jq -r '.timestamp // .ts' 2>/dev/null | cut -c1-19)

  echo "Time range: $first_time → $last_time"
fi

# Check for additional log files
if [ -d ~/.memex/logs ]; then
  echo
  echo "Additional logs:"
  ls -lh ~/.memex/logs/ 2>/dev/null || echo "  (empty)"
fi
```

## Maintenance

```bash
echo "=== Log Maintenance ==="
echo

# Show total size
if [ -f ./run.events.jsonl ]; then
  size=$(du -sh ./run.events.jsonl 2>/dev/null | cut -f1)
  echo "Current size: $size"
fi

echo
echo "Cleanup options:"
echo "  1. Archive old events:"
echo "     mv ./run.events.jsonl ./run.events.jsonl.archive"
echo
echo "  2. Compress archive:"
echo "     gzip ./run.events.jsonl.archive"
echo
echo "  3. Clear events (use with caution):"
echo "     > ./run.events.jsonl"
```

## Config Check

```bash
echo "=== Events Out Configuration ==="
echo

if grep -q '\[events_out\]' ~/.memex/config.toml ./config.toml 2>/dev/null; then
  echo "Events out is configured in config.toml"
  grep -A3 '\[events_out\]' ~/.memex/config.toml ./config.toml 2>/dev/null | head -5
else
  echo "Using default events_out settings"
  echo "  enabled: true"
  echo "  path: ./run.events.jsonl"
fi
```

## Quick Diagnostics

```bash
echo "=== Quick Diagnostics ==="
echo

# Common issues
if [ ! -f ./run.events.jsonl ]; then
  echo "⚠️  No run.events.jsonl found"
  echo "   → Check [events_out] enabled in config.toml"
  echo "   → Check path in [events_out] section"
fi

# Check if file is growing
if [ -f ./run.events.jsonl ]; then
  lines_1=$(wc -l < ./run.events.jsonl)
  sleep 2
  lines_2=$(wc -l < ./run.events.jsonl)

  if [ "$lines_2" -gt "$lines_1" ]; then
    echo "✅ Events are being written (active)"
  else
    echo "⚠️  No new events (inactive or disabled)"
  fi
fi
```
