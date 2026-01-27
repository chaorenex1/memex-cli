# Search Parameters Optimization Guide

## Overview

Fine-tune memory search behavior through parameters in `.claude/memory.local.md` or command-line arguments.

## Core Parameters

### search_limit

**Purpose**: Maximum number of search results to return

**Default**: 5

**Range**: 1-50

**Tuning guide**:
- **1-3**: Minimal context, fastest performance, high precision needed
- **5-10**: Balanced approach (recommended for most use cases)
- **10-20**: Comprehensive context, slower but more thorough
- **20+**: Deep research mode, may introduce noise

**Example scenarios**:
```yaml
# Quick lookups with precise context
search_limit: 3

# Standard development workflow
search_limit: 10

# Research-heavy projects
search_limit: 20
```

### min_score

**Purpose**: Minimum relevance threshold (0.0 to 1.0)

**Default**: 0.6

**Range**: 0.0-1.0

**Tuning guide**:
- **0.8-1.0**: Only highly relevant results, may miss useful context
- **0.6-0.8**: Balanced relevance (recommended)
- **0.4-0.6**: Broader context, may include tangential results
- **0.0-0.4**: Very permissive, high noise risk

**Example scenarios**:
```yaml
# Strict matching for critical operations
min_score: 0.8

# Standard balanced search
min_score: 0.6

# Exploratory research
min_score: 0.4
```

## Advanced Tuning

### Performance vs. Completeness Tradeoff

**High Performance** (faster searches, less context):
```yaml
search_limit: 3
min_score: 0.7
```

**High Completeness** (slower searches, more context):
```yaml
search_limit: 15
min_score: 0.5
```

### Domain-Specific Tuning

**Backend Development**:
```yaml
search_limit: 8
min_score: 0.65
# Good balance for API patterns and database queries
```

**Frontend Development**:
```yaml
search_limit: 5
min_score: 0.7
# UI patterns tend to be more specific
```

**DevOps/Infrastructure**:
```yaml
search_limit: 10
min_score: 0.6
# Commands and configs benefit from broader context
```

## Testing Your Settings

After changing parameters:

1. Restart Claude Code
2. Run test search:
   ```bash
   memex-cli search --query "your test query" \
     --limit <your_limit> \
     --min-score <your_score>
   ```
3. Evaluate results:
   - Too few results → Decrease `min_score` or increase `search_limit`
   - Too many irrelevant results → Increase `min_score` or decrease `search_limit`
   - Good balance → Keep current settings

## Monitoring Search Quality

Check `/view-memory-logs` to see actual search results being injected:
```
[2026-01-12 16:00:00] Memory inject: 5 results found (scores: 0.85, 0.72, 0.68, 0.61, 0.60)
```

**Analysis**:
- All scores above 0.6 → Good relevance
- Many scores near min_score → Consider raising threshold
- Scores clustered high (>0.8) → Can lower threshold to get more context

## Recommended Starting Points

**New projects** (limited memory):
```yaml
search_limit: 3
min_score: 0.7
```

**Established projects** (rich memory):
```yaml
search_limit: 10
min_score: 0.6
```

**Research/exploration**:
```yaml
search_limit: 15
min_score: 0.5
```

Adjust based on actual usage patterns observed in logs.
