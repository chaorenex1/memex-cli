# Knowledge Recording Strategies

## Overview

Best practices for effectively recording and curating knowledge in memex memory system.

## Automatic Recording

### What Gets Recorded Automatically

**PostToolUse Hook** records:
- **Write tool**: New file creation (full content)
- **Edit tool**: File modifications (changes made)
- **Bash tool**: Commands executed (except trivial ones like `ls`, `pwd`)

**Stop/SubagentStop Hooks** record:
- Complete conversation transcripts
- Extracted Q&A pairs from sessions
- Session metadata (duration, tool usage)

### What Doesn't Get Recorded

**Intentionally skipped**:
- Simple read operations (Read, Grep, Glob)
- Navigation commands (`cd`, `ls`, `pwd`)
- Tool calls that failed or were rejected
- Sensitive content (passwords, API keys - filtered by gatekeeper)

### Optimizing Automatic Recording

**Good practices**:
1. Use descriptive commit messages → Better query extraction
2. Comment code changes → Richer answer context
3. Complete tasks fully → Sessions record complete workflows
4. Use meaningful variable names → Searchable content

**Avoid**:
- Fragmented work → Incomplete sessions
- Vague descriptions → Poor query extraction
- Abandoned tasks → Partial knowledge recorded

## Manual Recording

### When to Record Manually

**Use `memex-cli record-candidate`**:
- External knowledge (docs, team discussions, Stack Overflow)
- Decision rationale not visible in code
- Troubleshooting steps that worked
- Performance tuning results
- Security considerations

**Example**:
```bash
memex-cli record-candidate \
  --query "Why did we choose PostgreSQL over MongoDB?" \
  --answer "Need ACID transactions for payment processing, complex joins for reporting" \
  --tags "architecture,database,decisions"
```

### Manual Recording Best Practices

**Good queries** (searchable, specific):
- ✅ "How to fix CORS errors in Next.js API routes?"
- ✅ "What's the rate limit for GitHub API?"
- ❌ "Fix error" (too vague)
- ❌ "Configuration" (too broad)

**Good answers** (actionable, complete):
- ✅ "Add `Access-Control-Allow-Origin: *` header in middleware.ts, restart dev server"
- ✅ "5000 requests/hour for authenticated, 60/hour for unauthenticated"
- ❌ "Add header" (incomplete)
- ❌ "See docs" (not actionable)

**Effective tags**:
- Technology: `typescript`, `react`, `postgres`
- Domain: `auth`, `payment`, `api`
- Type: `bug-fix`, `optimization`, `security`

## Knowledge Lifecycle

### Candidate → Verified Flow

1. **Candidate** (validation_level=0):
   - New knowledge recorded
   - Not yet proven useful
   - Lower priority in search results

2. **Hit recording**:
   - When candidate is referenced and helps solve problem
   - Use `memex-cli record-hit --qa-id <id>`
   - Increments hit count

3. **Automatic promotion**:
   - Gatekeeper evaluates hit count
   - High-hit candidates → validation_level=1 (Verified)
   - Frequently-used verified → level=2 (Confirmed)
   - Critical confirmed → level=3 (Gold Standard)

### Manual Promotion

**When to manually verify**:
- Imported knowledge from trusted sources
- Critical security patterns
- Team-agreed best practices

```bash
# Record directly as verified
memex-cli record-candidate --validation-level 1 \
  --query "Team's Git workflow" \
  --answer "Feature branch → PR → 2 approvals → Squash merge to main"
```

## Tagging Strategies

### Hierarchical Tags

**Pattern**: `category:subcategory:detail`

Examples:
- `lang:typescript:generics`
- `framework:react:hooks:useEffect`
- `infra:aws:s3:permissions`

### Cross-Cutting Tags

**Common dimensions**:
- **Urgency**: `urgent`, `p0`, `blocker`
- **Audience**: `team-lead`, `frontend-team`, `all`
- **Type**: `bug`, `feature`, `refactor`, `docs`
- **Status**: `active`, `deprecated`, `experimental`

### Tag Governance

**Best practices**:
1. Establish team tag taxonomy
2. Document common tags in project README
3. Review and consolidate tags periodically
4. Use consistent casing (`kebab-case` recommended)

## Quality Gates

### What Gatekeeper Filters

**Automatically rejected**:
- Contains `password`, `secret`, `token`, `api_key`
- File paths to sensitive locations (`.env`, `credentials.json`)
- Personal information (emails, phone numbers)
- Trivial changes (whitespace, comments only)

**Automatically approved**:
- Non-trivial code changes
- Configuration updates
- Command executions with clear purpose
- Documentation additions

### Overriding Gatekeeper

If important knowledge gets filtered:

1. Review gatekeeper logs: `/view-memory-logs`
2. Redact sensitive parts manually
3. Record manually:
   ```bash
   memex-cli record-candidate \
     --query "Database migration procedure" \
     --answer "Run migrate.sh with [REDACTED] credentials, verify with health check"
   ```

## Bulk Recording Strategies

### Session Recording

**Good session practices**:
- Work on one topic per session → Focused transcripts
- Summarize at end → Better extraction
- Use clear prompts → Searchable queries

**Example end-of-session summary**:
```
Completed JWT authentication implementation:
- Created auth middleware in middleware/auth.ts
- Added token validation logic
- Wrote tests in __tests__/auth.test.ts
All tests passing, ready for review.
```

### Batch Imports

**Importing external knowledge**:
```bash
# From documentation notes
cat docs/api-patterns.md | while read -r line; do
  memex-cli record-candidate \
    --query "API pattern: $line" \
    --answer "$(grep -A 5 "$line" docs/api-patterns.md)" \
    --tags "api,patterns,docs"
done
```

## Monitoring and Maintenance

### Regular Reviews

**Weekly**:
- Check `/view-memory-logs` for recording failures
- Review high-hit candidates for promotion
- Validate search quality with `/test-memory`

**Monthly**:
- Export memory statistics
- Identify gaps in knowledge coverage
- Retire deprecated knowledge

### Cleaning Up

**Remove obsolete knowledge**:
```bash
# Mark as deprecated
memex-cli update-qa --qa-id <id> --tags "deprecated,archived"

# Or delete entirely
memex-cli delete-qa --qa-id <id>
```

## Advanced Patterns

### Project-Specific Memory

Use tags to scope memory by project:
```yaml
# In .claude/memory.local.md
---
default_tags: ["project:memex-cli", "team:backend"]
---
```

All recordings automatically get project tags.

### Contextual Recording

**Before major changes**:
```bash
# Record current state
memex-cli record-candidate \
  --query "Why auth system designed this way (before refactor)" \
  --answer "$(cat docs/auth-design.md)"
```

**After changes**:
```bash
# Record migration path
memex-cli record-candidate \
  --query "How to migrate from old auth to new JWT system" \
  --answer "Step 1: ..., Step 2: ..., Breaking changes: ..."
```

## Summary

**Automatic recording**: Let hooks capture day-to-day work
**Manual recording**: Fill gaps with external knowledge and rationale
**Tagging**: Use consistent, hierarchical tags for discoverability
**Lifecycle**: Trust the candidate → verified promotion flow
**Maintenance**: Regular reviews to keep memory healthy

Focus on quality over quantity - one well-recorded piece of knowledge is worth ten vague entries.
