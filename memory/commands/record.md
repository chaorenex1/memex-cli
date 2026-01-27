---
name: record
description: Manually record knowledge to memory
allowed-tools: ["Bash", "AskUserQuestion"]
---

# Record Knowledge

Manually save a question-answer pair to memory.

## Configuration

Recording behavior is controlled by `[candidate_extract]` section in config.toml:

```toml
[candidate_extract]
max_candidates = 10           # Max candidates to extract
max_answer_chars = 2000       # Truncate long answers
min_answer_chars = 100        # Min answer length
redact = true                 # Redact sensitive info
strict_secret_block = true    # Block secrets completely
confidence = 0.45             # Extraction confidence threshold
```

## Input

```bash
# Get user input
read -p "Question: " question
read -p "Answer: " answer
read -p "Tags (comma separated) [optional]: " tags
read -p "Namespace [default] : " namespace
namespace=${namespace:-default}
```

## Record as Candidate

```bash
# Record for later validation
memex-cli record-candidate \
  --query "$question" \
  --answer "$answer" \
  --tags "$tags" \
  --namespace "$namespace"
```

## Record as Verified

If you want to mark as already verified:

```bash
# First record as candidate, then mark as hit
qa_id=$(memex-cli record-candidate --query "$question" --answer "$answer" --tags "$tags" | grep -o 'qa-[0-9]*')

memex-cli record-hit \
  --qa-id "$qa_id" \
  --namespace "$namespace" \
  --hit-type "manual_verification"
```

## Example Usage

```bash
# Simple record
memex-cli record-candidate \
  --query "How to implement JWT in Rust?" \
  --answer "Use jsonwebtoken crate with HMAC secret" \
  --tags "rust,jwt,auth"

# Record from external source
memex-cli record-candidate \
  --query "PostgreSQL connection pool best practices" \
  --answer "Use deadpool with max_size=15, min_idle=5, connection_timeout=30s" \
  --tags "database,postgres,performance"

# Record with namespace
memex-cli record-candidate \
  --query "Company API endpoints" \
  --answer "API base: https://api.company.com/v2" \
  --namespace "company-docs"
```

## When to Record

**Good candidates**:
- Solutions to hard problems
- Configuration patterns
- API documentation
- Debugging steps
- Best practices

**Avoid recording**:
- Temporary workarounds
- Environment-specific data
- Sensitive information (keys, passwords)
- Trivial/obvious facts

## Record Workflow

```
1. /record
   ↓
2. Enter question and answer
   ↓
3. Stored as candidate (validation_level=0)
   ↓
4. Use solution in future sessions
   ↓
5. System promotes to verified when used successfully
```

## Verification

After recording, verify it was saved:

```bash
memex-cli search --query "<your question>" --limit 1
```
