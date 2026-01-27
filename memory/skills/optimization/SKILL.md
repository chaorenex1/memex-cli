---
name: Optimization
description: Guide for tuning memory performance. Use when user asks "optimize memory", "tune performance", "improve search", or mentions memory speed/accuracy.
version: 1.0.0
---

# Memory Optimization Guide

Tune memory plugin for optimal performance and accuracy.

## Key Parameters

| Parameter | Effect | Range | Default |
|-----------|--------|-------|---------|
| `search_limit` | Results injected | 1-20 | 6 |
| `min_score` | Relevance threshold | 0.0-1.0 | 0.2 |

## Configuration Presets

### Precision Mode (High Accuracy)

```toml
[memory]
search_limit = 3
min_score = 0.4
```

**Use when**:
- Exact matches needed
- Domain-specific questions
- Small knowledge base

**Trade-off**: May miss relevant but lower-scored results

### Balanced Mode (Default)

```toml
[memory]
search_limit = 6
min_score = 0.2
```

**Use when**:
- General development
- Mixed topics
- Medium knowledge base

### Exploration Mode (Broad Coverage)

```toml
[memory]
search_limit = 10
min_score = 0.1
```

**Use when**:
- Research and learning
- Discovering connections
- Large knowledge base

**Trade-off**: More noise, slower responses

### Performance Mode (Fastest)

```toml
[memory]
search_limit = 3
min_score = 0.3
```

**Use when**:
- Speed critical
- Common/repeated tasks
- Well-organized knowledge

## Tuning Strategy

### 1. Assess Current Performance

```bash
# Check search speed
time /search

# Review result quality
# Do results match your intent?
# Are scores generally high (>0.7)?
```

### 2. Identify Bottleneck

| Issue | Parameter to Adjust |
|-------|---------------------|
| Too many irrelevant results | Increase `min_score` |
| Missing relevant results | Decrease `min_score` |
| Slow responses | Decrease `search_limit` |
| Insufficient context | Increase `search_limit` |

### 3. Apply Changes

Edit `~/.memex/config.toml` (global) or `./config.toml` (project):

```toml
[memory]
search_limit = <new_value>
min_score = <new_value>
```

**Important**: Reload config or restart service after changing.

### 4. Validate

```bash
# Test with same queries
/search → your query

# Compare results quality and speed
```

## Iterative Tuning

```
Start with defaults (limit=6, score=0.2)
    ↓
Use for a week, observe behavior
    ↓
Adjust ONE parameter at a time
    ↓
Test with real queries
    ↓
Keep or revert
    ↓
Repeat
```

## Performance Tips

### Reduce Search Time

1. **Lower search_limit**: 3-5 results is usually sufficient
2. **Raise min_score**: 0.7+ reduces noise
3. **Use specific queries**: Better queries = faster, better results

### Improve Result Quality

1. **Record quality knowledge**: Garbage in, garbage out
2. **Use consistent terminology**: Helps matching
3. **Reference QA IDs**: Creates feedback loop for validation

### Manage Memory Growth

1. **Regular cleanup**: Archive old entries
2. **Validation levels**: Focus on level 2+ (confirmed)
3. **Namespace separation**: Organize by project/domain

## Monitoring

### Check Performance

```bash
# Time your searches
time memex-cli search --query "test" --limit 5

# Review scores
/search → look at score distribution
```

### Healthy Indicators

- Average score: 0.3-0.6 (depends on min_score setting)
- Search time: <1 second
- Results: 2-5 relevant items

### Warning Signs

- Average score < 0.3 → Consider raising min_score
- Search time > 3s → Lower search_limit
- Zero results → Lower min_score

## Advanced: Additional Parameters

Beyond `[memory]` section, consider tuning:

```toml
[prompt_inject]
max_items = 10                    # Max items to inject
max_answer_chars = 1000           # Truncate long answers

[gatekeeper]
min_level_inject = 2              # Min validation level (0-3)
min_trust_show = 0.40             # Min trust score to show
```

## Quick Reference

| Goal | search_limit | min_score |
|------|--------------|-----------|
| Fastest | 3 | 0.4 |
| Balanced (default) | 6 | 0.2 |
| Comprehensive | 10 | 0.1 |
| Precision | 3 | 0.5 |
| Exploration | 15 | 0.1 |

## Common Mistakes

1. **Changing both parameters at once** → Hard to identify cause
2. **Setting min_score too high** → Misses relevant results
3. **Setting search_limit too high** → Slower, more noise
4. **Not restarting after changes** → Changes don't take effect

## Commands for Tuning

```
/search     - Test search with current settings
/logs        - Check performance in logs
/setup      - Apply new settings
```
