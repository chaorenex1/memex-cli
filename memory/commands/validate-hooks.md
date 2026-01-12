---
name: validate-hooks
description: Validate memory hooks configuration and dependencies
allowed-tools: ["Bash", "Read"]
---

# Validate Hooks Configuration

Comprehensive validation of hooks setup, scripts, and dependencies.

## Validation Steps

### 1. Validate hooks.json Structure

```bash
echo "=== Validating hooks.json ==="

# Check file exists
if [ ! -f "memory/hooks/hooks.json" ]; then
  echo "❌ hooks.json not found at memory/hooks/hooks.json"
  exit 1
fi

# Validate JSON syntax
if python -m json.tool memory/hooks/hooks.json > /dev/null 2>&1; then
  echo "✅ hooks.json is valid JSON"
else
  echo "❌ hooks.json has syntax errors"
  python -m json.tool memory/hooks/hooks.json 2>&1 | head -20
  exit 1
fi

# Check required structure
python << 'EOF'
import json
with open('memory/hooks/hooks.json') as f:
    data = json.load(f)

# Check wrapper format
if 'hooks' not in data:
    print("❌ Missing 'hooks' wrapper field")
    exit(1)

hooks_data = data['hooks']

# Check event types
events = ['SessionStart', 'UserPromptSubmit', 'PostToolUse', 'Stop',
          'SubagentStop', 'PreCompact', 'SessionEnd']

for event in events:
    if event in hooks_data:
        print(f"✅ {event} hook configured")
    else:
        print(f"⚠️  {event} hook not configured")

print("✅ hooks.json structure valid")
EOF
```

### 2. Validate Script References

```bash
echo -e "\n=== Validating Script References ==="

# Extract all script paths from hooks.json
python << 'EOF'
import json
import re

with open('memory/hooks/hooks.json') as f:
    data = json.load(f)

scripts = set()
content = json.dumps(data)

# Find all script references (after ${CLAUDE_PLUGIN_ROOT}/)
pattern = r'\$\{CLAUDE_PLUGIN_ROOT\}/scripts/([a-zA-Z0-9_-]+\.py)'
for match in re.finditer(pattern, content):
    scripts.add(match.group(1))

# Check each script exists
all_found = True
for script in sorted(scripts):
    script_path = f'memory/scripts/{script}'
    if os.path.exists(script_path):
        print(f"✅ {script} exists")
    else:
        print(f"❌ {script} not found")
        all_found = False

if all_found:
    print("✅ All referenced scripts exist")
else:
    print("❌ Some scripts are missing")
    exit(1)
EOF
```

### 3. Validate Python Scripts Syntax

```bash
echo -e "\n=== Validating Python Script Syntax ==="

# Check each script for syntax errors
for script in memory/scripts/*.py; do
  script_name=$(basename "$script")
  if python -m py_compile "$script" 2>/dev/null; then
    echo "✅ $script_name syntax OK"
  else
    echo "❌ $script_name has syntax errors:"
    python -m py_compile "$script" 2>&1 | head -10
  fi
done
```

### 4. Validate Python Dependencies

```bash
echo -e "\n=== Validating Python Dependencies ==="

# Check requirements file exists
if [ -f "memory/requirements-http.txt" ]; then
  echo "✅ requirements-http.txt found"

  # Check each requirement
  while read -r package; do
    # Skip empty lines and comments
    [[ -z "$package" || "$package" =~ ^# ]] && continue

    # Extract package name (before ==, >=, etc.)
    pkg_name=$(echo "$package" | sed 's/[>=<].*//')

    if python -c "import $pkg_name" 2>/dev/null; then
      echo "✅ $pkg_name installed"
    else
      echo "❌ $pkg_name not installed"
      echo "   Install with: pip install $package"
    fi
  done < memory/requirements-http.txt
else
  echo "⚠️  requirements-http.txt not found"
fi

# Check critical imports
for module in sys json subprocess os; do
  if python -c "import $module" 2>/dev/null; then
    echo "✅ $module (builtin) available"
  else
    echo "❌ $module (builtin) missing - Python installation issue"
  fi
done
```

### 5. Validate Script Permissions (Linux/macOS)

```bash
echo -e "\n=== Validating Script Permissions ==="

if [[ "$OSTYPE" == "linux-gnu"* || "$OSTYPE" == "darwin"* ]]; then
  for script in memory/scripts/*.py; do
    if [ -x "$script" ]; then
      echo "✅ $(basename $script) is executable"
    else
      echo "⚠️  $(basename $script) not executable (not required if using 'python script.py')"
    fi
  done
else
  echo "⚠️  Windows detected - script permissions not applicable"
fi
```

### 6. Validate ${CLAUDE_PLUGIN_ROOT} Usage

```bash
echo -e "\n=== Validating Portable Path References ==="

# Check hooks.json uses ${CLAUDE_PLUGIN_ROOT}
if grep -q '\${CLAUDE_PLUGIN_ROOT}' memory/hooks/hooks.json; then
  echo "✅ hooks.json uses \${CLAUDE_PLUGIN_ROOT} for portability"

  # Count usages
  count=$(grep -o '\${CLAUDE_PLUGIN_ROOT}' memory/hooks/hooks.json | wc -l)
  echo "   Found $count references"
else
  echo "❌ hooks.json doesn't use \${CLAUDE_PLUGIN_ROOT}"
  echo "   Scripts won't work when plugin is installed in different locations"
fi

# Check for hardcoded paths (anti-pattern)
if grep -E '"/[^$].*scripts/' memory/hooks/hooks.json; then
  echo "⚠️  Found hardcoded absolute paths in hooks.json"
  echo "   Replace with: \${CLAUDE_PLUGIN_ROOT}/scripts/..."
fi
```

### 7. Validate Timeout Values

```bash
echo -e "\n=== Validating Hook Timeouts ==="

python << 'EOF'
import json

with open('memory/hooks/hooks.json') as f:
    data = json.load(f)

# Recommended timeouts
recommendations = {
    'SessionStart': (5, 10),      # Fast startup
    'SessionEnd': (5, 30),         # Quick cleanup
    'UserPromptSubmit': (30, 300), # Memory search
    'PostToolUse': (30, 300),      # Record knowledge
    'Stop': (30, 300),             # Session recording
}

for event, hooks_list in data['hooks'].items():
    for hook_config in hooks_list:
        for hook in hook_config.get('hooks', []):
            timeout = hook.get('timeout', 60)  # default
            min_rec, max_rec = recommendations.get(event, (10, 600))

            if min_rec <= timeout <= max_rec:
                print(f"✅ {event} timeout={timeout}s (within recommended {min_rec}-{max_rec}s)")
            elif timeout < min_rec:
                print(f"⚠️  {event} timeout={timeout}s (may be too short, recommend >={min_rec}s)")
            else:
                print(f"⚠️  {event} timeout={timeout}s (may be too long, recommend <={max_rec}s)")
EOF
```

### 8. Validate Settings File (if exists)

```bash
echo -e "\n=== Validating Settings File ==="

if [ -f ".claude/memory.local.md" ]; then
  echo "✅ Settings file found"

  # Extract and validate YAML frontmatter
  python << 'EOF'
import re

with open('.claude/memory.local.md') as f:
    content = f.read()

# Extract frontmatter
match = re.search(r'^---\n(.*?)\n---', content, re.DOTALL)
if not match:
    print("❌ No YAML frontmatter found")
    exit(1)

frontmatter = match.group(1)

# Check for common fields
required = ['enabled']
optional = ['base_url', 'api_key', 'search_limit', 'min_score', 'memex_cli_path']

for field in required:
    if f'{field}:' in frontmatter:
        print(f"✅ Required field: {field}")
    else:
        print(f"❌ Missing required field: {field}")

for field in optional:
    if f'{field}:' in frontmatter:
        print(f"✅ Optional field: {field}")

print("✅ Settings frontmatter structure valid")
EOF

  # Validate YAML syntax
  if python -c "import yaml; yaml.safe_load(open('.claude/memory.local.md').read().split('---')[1])" 2>/dev/null; then
    echo "✅ Settings YAML syntax valid"
  else
    echo "❌ Settings YAML has syntax errors"
  fi

else
  echo "⚠️  No settings file at .claude/memory.local.md"
  echo "   Hooks will use default configuration"
  echo "   Run /setup-memex to create one"
fi
```

### 9. Test Hook Execution

```bash
echo -e "\n=== Testing Hook Execution ==="

# Test memory-inject.py with minimal input
echo "Testing memory-inject.py..."
echo '{"user_prompt":"test","session_id":"validation-test"}' | \
  python memory/scripts/memory-inject.py > /tmp/hook-test-output.txt 2>&1

if [ $? -eq 0 ]; then
  echo "✅ memory-inject.py executed successfully"
else
  echo "❌ memory-inject.py failed:"
  cat /tmp/hook-test-output.txt | head -20
fi
```

## Validation Summary

```bash
echo -e "\n=== Validation Summary ==="

# Count issues
echo "Review the output above for:"
echo "  ✅ = Pass"
echo "  ⚠️  = Warning (optional or non-critical)"
echo "  ❌ = Fail (must fix)"

echo -e "\nCommon fixes:"
echo "  - Missing scripts: Check hooks/hooks.json references correct paths"
echo "  - Python errors: Install dependencies with: pip install -r memory/requirements-http.txt"
echo "  - Syntax errors: Review script syntax with Python linter"
echo "  - Permission issues: chmod +x memory/scripts/*.py (Linux/macOS)"

echo -e "\nNext steps:"
echo "  1. Fix any ❌ errors above"
echo "  2. Restart Claude Code to reload hooks"
echo "  3. Test with: /test-memory"
```

## Output Format

Present results in clear sections with:
- ✅ for passed checks
- ⚠️ for warnings (non-critical)
- ❌ for failures (must fix)

Provide actionable recommendations for each failure.
