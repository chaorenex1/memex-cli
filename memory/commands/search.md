---
name: search
description: Manual memory search
allowed-tools: ["Bash", "AskUserQuestion"]
---

# Search Memory

Manually search the memory service for past knowledge.

## Parameters

```bash
# Get search query from user
read -p "Enter search query: " query

# Optional: get parameters
read -p "Limit [6]: " limit
limit=${limit:-6}

read -p "Min score [0.2]: " min_score
min_score=${min_score:-0.2}

read -p "Format [markdown]: " format
format=${format:-markdown}
```

## Execute Search

```bash
memex-cli search \
  --query "$query" \
  --limit "$limit" \
  --min-score "$min_score" \
  --format "$format"
```

## Result Interpretation

**Output includes**:
- `qa_id`: Unique identifier (e.g., `qa-12345`)
- `question`: The question that was stored
- `answer`: The answer/solution
- `score`: Relevance (0-1, higher is better)
- `validation_level`: Confidence level (0-3)
  - 0: Candidate (unverified)
  - 1: Verified (executed successfully)
  - 2: Confirmed (multiple successes)
  - 3: Gold standard (highly reliable)

## Example Usage

```bash
# Basic search
memex-cli search --query "JWT authentication" --limit 5

# High precision
memex-cli search --query "rate limiting" --min-score 0.8 --limit 3

# JSON output for parsing
memex-cli search --query "database" --format json | jq '.results[]'

# Search recent candidates
memex-cli search --query "recent" --validation-level 0
```

## Search Tips

1. **Be specific**: Use exact terms for better matches
2. **Adjust min_score**:
   - 0.8+ for exact matches
   - 0.4-0.6 for related topics (default 0.2 is permissive)
   - 0.2-0.4 for broad exploration
3. **Use validation_level** to filter by confidence
4. **Reference QA IDs** in prompts: `Based on [qa-12345], how do I...`

## Configuration

Search behavior is controlled by `[memory]` section in config.toml:

```toml
[memory]
search_limit = 6        # Max results (default)
min_score = 0.2         # Min relevance (default)
```

## Next Actions

- **Found useful info**: Reference it in next prompt with `[QA_REF qa-xxxxx]`
- **Want to record**: Use `/record` to save new knowledge
- **Results poor**: Lower min_score or try different search terms
