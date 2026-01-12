# Example Memory Queries

## Common Search Patterns

### By Technology

```bash
# Frontend frameworks
memex-cli search --query "React hooks useEffect" --limit 5

# Backend frameworks
memex-cli search --query "Express middleware authentication" --limit 5

# Databases
memex-cli search --query "PostgreSQL query optimization" --limit 5
```

### By Problem Type

```bash
# Debugging
memex-cli search --query "fix CORS error" --limit 10 --min-score 0.5

# Performance
memex-cli search --query "optimize bundle size webpack" --limit 5

# Security
memex-cli search --query "prevent SQL injection" --limit 5 --min-score 0.7
```

### By Task

```bash
# Implementation
memex-cli search --query "implement JWT authentication" --limit 5

# Configuration
memex-cli search --query "configure Docker multi-stage build" --limit 3

# Testing
memex-cli search --query "write integration tests API" --limit 5
```

## Recording Examples

### Basic Recording

```bash
# Simple Q&A
memex-cli record-candidate \
  --query "How to enable CORS in Express?" \
  --answer "Use cors middleware: app.use(cors({ origin: 'https://example.com' }))"

# With tags
memex-cli record-candidate \
  --query "Git rebase vs merge - when to use?" \
  --answer "Use rebase for feature branches to keep linear history, merge for long-lived branches" \
  --tags "git,workflow,best-practices"

# With metadata
memex-cli record-candidate \
  --query "Team's API versioning strategy" \
  --answer "Use URL versioning: /api/v1/*, /api/v2/* - easier for clients" \
  --tags "api,architecture,decisions" \
  --metadata '{"decision_date":"2026-01-12","team":"backend"}'
```

### Advanced Recording

```bash
# Multi-line answer with here-doc
memex-cli record-candidate \
  --query "Deploy Node.js app to production" \
  --answer "$(cat <<'EOF'
1. Build: npm run build
2. Set env vars: NODE_ENV=production
3. Start with PM2: pm2 start dist/server.js
4. Configure nginx reverse proxy
5. Enable SSL with certbot
EOF
  )" \
  --tags "deployment,nodejs,production"

# From file content
memex-cli record-candidate \
  --query "Database schema for user authentication" \
  --answer "$(cat schema/users.sql)" \
  --tags "database,schema,auth"
```

## Hit Recording Examples

```bash
# Mark candidate as useful
memex-cli record-hit \
  --qa-id "qa-abc123" \
  --hit-type "manual_verification"

# Record hit with context
memex-cli record-hit \
  --qa-id "qa-def456" \
  --hit-type "solved_problem" \
  --metadata '{"issue":"#123","success":true}'
```

## Session Recording Examples

```bash
# Record current session
memex-cli record-session \
  --session-id "$(cat ~/.claude/session-id)" \
  --transcript-path "$(find ~/.claude -name 'transcript-*.txt' | tail -1)"

# Record with custom tags
memex-cli record-session \
  --session-id "my-session-123" \
  --transcript-path "/path/to/transcript.txt" \
  --tags "pair-programming,refactoring"
```

## Format-Specific Queries

### JSON Output

```bash
memex-cli search --query "API error handling" --format json | jq '.results[0]'
```

**Example output**:
```json
{
  "qa_id": "qa-123",
  "query": "How to handle API errors in Express?",
  "answer": "Use centralized error middleware...",
  "score": 0.87,
  "validation_level": 1,
  "tags": ["express", "error-handling", "api"]
}
```

### Markdown Output

```bash
memex-cli search --query "Docker best practices" --format markdown
```

**Example output**:
```markdown
### 📚 Search Results for "Docker best practices"

**[qa-789]** Q: Docker multi-stage build benefits?
A: Reduces image size by 70%, separates build and runtime deps...
Score: 0.92 | Tags: docker, optimization
---

**[qa-456]** Q: Docker layer caching strategy?
A: Order Dockerfile commands from least to most frequently changed...
Score: 0.85 | Tags: docker, performance
---
```

## Advanced Search Filters

```bash
# By validation level
memex-cli search --query "authentication" --validation-level 1  # Verified only

# By tags
memex-cli search --query "database" --tags "postgresql,optimization"

# By date range
memex-cli search --query "recent fixes" --since "2026-01-01"

# Combined filters
memex-cli search \
  --query "API design patterns" \
  --tags "architecture,api" \
  --validation-level 1 \
  --limit 10 \
  --min-score 0.7
```

## Troubleshooting Queries

```bash
# Test connectivity
memex-cli search --query "test" --limit 1

# Verbose output
memex-cli search --query "debug" --verbose

# Check service health
curl http://localhost:8080/health
```

## Integration Examples

### In Claude Prompts

```
Claude, search our memory for "rate limiting strategies"
and implement the recommended approach.
```

### In Shell Scripts

```bash
#!/bin/bash
# Auto-record command executions

execute_and_record() {
  local cmd="$1"
  local description="$2"

  # Execute command
  output=$(eval "$cmd" 2>&1)
  exit_code=$?

  # Record if successful
  if [ $exit_code -eq 0 ]; then
    memex-cli record-candidate \
      --query "$description" \
      --answer "Command: $cmd\nOutput: $output" \
      --tags "automation,shell,$(basename $SHELL)"
  fi

  return $exit_code
}

# Usage
execute_and_record "docker build -t myapp ." "Build Docker image for myapp"
```

### In Git Hooks

```bash
#!/bin/bash
# .git/hooks/post-commit
# Record commit messages as memory

commit_msg=$(git log -1 --pretty=%B)
commit_hash=$(git rev-parse HEAD)
files_changed=$(git diff-tree --no-commit-id --name-only -r HEAD)

memex-cli record-candidate \
  --query "Recent commit: $commit_msg" \
  --answer "Files changed: $files_changed" \
  --tags "git,commit,$commit_hash" \
  --metadata "{\"commit\":\"$commit_hash\",\"date\":\"$(date -I)\"}"
```

## Common Patterns Summary

| Use Case | Command Pattern |
|----------|----------------|
| Quick lookup | `search --query "..." --limit 3` |
| Research | `search --query "..." --limit 15 --min-score 0.5` |
| Precise match | `search --query "..." --min-score 0.8` |
| Record simple | `record-candidate --query "..." --answer "..."` |
| Record complex | `record-candidate --query "..." --answer "$(cat file)" --tags "..."` |
| Mark useful | `record-hit --qa-id "..." --hit-type "solved_problem"` |
| Record session | `record-session --session-id "..." --transcript-path "..."` |
