---
name: reset
description: Reset or clean up memex-cli state
allowed-tools: ["Bash", "AskUserQuestion"]
---

# Reset Memory

Reset or clean up memex-cli memory state and configuration.

## Options

Ask user what they want to reset:

**What would you like to reset?**
- "Logs only - Clear event logs"
- "Config - Remove configuration"
- "All - Complete reset (use with caution)"

## 1. Reset Logs

```bash
echo "Clearing log files..."

# Archive first (safety)
if [ -f ./run.events.jsonl ]; then
  mv ./run.events.jsonl ./run.events.jsonl.backup 2>/dev/null
  echo "✅ Logs backed up to ./run.events.jsonl.backup"
  echo "   New log file will be created on next run"
else
  echo "⚠️  No run.events.jsonl found"
fi

# Clear log directory if exists
if [ -d ~/.memex/logs ]; then
  rm -rf ~/.memex/logs/* 2>/dev/null
  echo "✅ Log directory cleared"
fi
```

## 2. Reset Config

```bash
echo "Removing configuration..."

# Global config
if [ -f ~/.memex/config.toml ]; then
  # Backup first
  cp ~/.memex/config.toml ~/.memex/config.toml.backup
  # Remove memory section only (preserve other config)
  if command -v python &> /dev/null; then
    python << 'PYTHON'
import tomllib
import sys

try:
    with open('$HOME/.memex/config.toml', 'r') as f:
        config = tomllib.load(f)

    # Remove memory section
    if 'memory' in config:
        del config['memory']

    # Write back
    with open('$HOME/.memex/config.toml', 'w') as f:
        import toml
        toml.dump(config, f)

    print("✅ [memory] section removed from config")
except Exception as e:
    print(f"⚠️  Could not update config: {e}")
    sys.exit(1)
PYTHON
  fi
fi

# Project config
if [ -f ./config.toml ]; then
  # Backup
  cp ./config.toml ./config.toml.backup
  # Remove memory section
  sed -i '/^\[memory\]/,/^\[/{ /^\[memory\]/d; /^[^[]/d; }' ./config.toml 2>/dev/null
  echo "✅ Project config backed up to ./config.toml.backup"
else
  echo "⚠️  No config.toml found"
fi
```

## 3. Disable Memory

```bash
echo "Disabling memory features..."

# Update config to disable memory
if [ -f ~/.memex/config.toml ]; then
  if command -v python &> /dev/null; then
    python << 'PYTHON'
import tomllib
import sys

try:
    with open('$HOME/.memex/config.toml', 'r') as f:
        config = tomllib.load(f)

    # Disable memory
    if 'memory' not in config:
        config['memory'] = {}
    config['memory']['enabled'] = False

    # Write back
    with open('$HOME/.memex/config.toml', 'w') as f:
        import toml
        toml.dump(config, f)

    print("✅ Memory disabled in config.toml")
except Exception as e:
    print(f"⚠️  Could not update config: {e}")
    sys.exit(1)
PYTHON
  fi
elif [ -f ./config.toml ]; then
  # Add disabled memory section to project config
  cat >> ./config.toml <<'EOF'

[memory]
enabled = false
EOF
  echo "✅ Memory disabled in ./config.toml"
fi
```

## 4. Complete Reset

```bash
echo "⚠️  COMPLETE RESET"
echo "This will:"
echo "  - Clear all logs"
echo "  - Remove memory configuration"
echo "  - Disable memory features"
echo

read -p "Are you sure? (yes/no): " confirm

if [ "$confirm" = "yes" ]; then
  # Archive logs
  [ -f ./run.events.jsonl ] && mv ./run.events.jsonl ./run.events.jsonl.backup
  [ -f ~/.memex/config.toml ] && cp ~/.memex/config.toml ~/.memex/config.toml.backup
  [ -f ./config.toml ] && cp ./config.toml ./config.toml.backup

  # Disable memory
  if [ -f ~/.memex/config.toml ]; then
    python -c "
import tomllib, toml
config = tomllib.load(open('$HOME/.memex/config.toml', 'rb'))
config['memory']['enabled'] = False
toml.dump(config, open('$HOME/.memex/config.toml', 'w'))
" 2>/dev/null
  fi

  echo "✅ Reset complete"
  echo
  echo "Backups created:"
  echo "  - ~/.memex/config.toml.backup"
  echo "  - ./config.toml.backup (if existed)"
  echo "  - ./run.events.jsonl.backup (if existed)"
  echo
  echo "To reconfigure: /setup"
else
  echo "Reset cancelled"
fi
```

## Reset Confirmation

After any reset operation, show summary:

```
=== Reset Summary ===

Logs:        Cleared/Archived
Config:      Removed/Disabled
Memory:      Disabled

Next steps:
  1. Reconfigure: /setup
  2. Test: /test
  3. Verify: /health
```

## Restore from Backup

```bash
echo "=== Restore from Backup ==="

# Restore global config
if [ -f ~/.memex/config.toml.backup ]; then
  mv ~/.memex/config.toml.backup ~/.memex/config.toml
  echo "✅ Global config restored"
fi

# Restore project config
if [ -f ./config.toml.backup ]; then
  mv ./config.toml.backup ./config.toml
  echo "✅ Project config restored"
fi

# Restore logs
if [ -f ./run.events.jsonl.backup ]; then
  mv ./run.events.jsonl.backup ./run.events.jsonl
  echo "✅ Events log restored"
fi
```
