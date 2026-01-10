# CLAUDE.md

This file provides guidance for Claude Code when working with this repository.

## Project Overview

Memex-CLI is a Rust-based CLI shell wrapper with memory, replay, and resume capabilities:
- Records execution to `run.events.jsonl` (audit/replay friendly)
- Supports `replay` and `resume` based on run IDs
- Memory retrieval and context injection
- Tool/policy approval gates
- Cross-platform support (Windows, macOS, Linux)

## Terminology

### run_id
**Definition**: Unique identifier for a single execution (primary term)

**Usage**:
- Core data structures: `WrapperEvent.run_id`, `ToolEvent.run_id`, `ReplayRun.run_id`
- Persisted in `run.events.jsonl` files
- Used for replay/resume functionality
- Primary key for execution tracking and audit

### session_id
**Definition**: Context-dependent identifier with different meanings in different layers

**Usage 1 - HTTP Server Instance Identifier**:
- Identifies the HTTP server process (NOT a single execution)
- Used in: `AppState.session_id`, `HealthResponse.session_id`
- Written to state file: `~/.memex/servers/memex-{session_id}.state`
- One server can handle multiple runs with different run_ids

**Usage 2 - CLI Parameter Alias for run_id**:
- In CLI args (e.g., `RecordSessionArgs.session_id`), internally treated as `run_id`
- Used for integration with external systems (e.g., Claude's session_id)
- Provides backward compatibility

**Usage 3 - Backend Output Compatibility**:
- Some backends (e.g., Gemini) return `session_id` instead of `run_id`
- Extracted via `run_id_extract.rs` which supports multiple field names

**Design Principle**: Use `run_id` in core engine and documentation; use `session_id` only in specific contexts (HTTP server, CLI aliases, backend compatibility). See `docs/TERMINOLOGY.md` for detailed explanations.

## Workspace Structure

```
memex-cli/
├── core/              # Core execution engine and domain logic (memex-core)
├── plugins/           # Backend, memory, policy, gatekeeper implementations (memex-plugins)
├── cli/               # Binary entry point and TUI (memex-cli)
├── config.toml        # Default configuration
├── .env.online        # Online environment variables
└── .env.offline       # Offline environment variables
```

## Build Commands

```bash
# Build release binary
cargo build -p memex-cli --release

# Size-optimized release
cargo build -p memex-cli --profile size-release

# Development build
cargo build -p memex-cli

# Run tests
cargo test --workspace

# Format code
cargo fmt --all

# Lint with clippy
cargo clippy --workspace --all-targets -- -D warnings
```

## Key Entry Points

- `cli/src/main.rs` - Binary entry point, argument parsing, command dispatch
- `cli/src/app.rs` - Application orchestration, config merging, TUI/standard flow selection
- `cli/src/commands/cli.rs` - CLI argument definitions (RunArgs, ReplayArgs, ResumeArgs, SearchArgs, RecordCandidateArgs, RecordHitArgs, RecordSessionArgs)
- `cli/src/commands/memory.rs` - Memory service CLI command handlers (search, record-candidate, record-hit, record-session)
- `core/src/api.rs` - Public API re-exports
- `core/src/engine/run.rs` - Main execution orchestration
- `plugins/src/factory.rs` - Plugin instantiation (memory, runner, policy, gatekeeper, backend)

## Architecture

### Core Traits (in `core/`)
- `BackendStrategy` - Abstracts codecli/aiservice backends
- `RunnerPlugin` - Process execution
- `PolicyPlugin` - Tool approval logic
- `MemoryPlugin` - Memory operations
- `GatekeeperPlugin` - Quality gates for memory persistence

### Plugin Implementations (in `plugins/`)
- `CodeCliBackendStrategy` / `AiServiceBackendStrategy` - Backend implementations
- `CodeCliRunnerPlugin` / `ReplayRunnerPlugin` - Runner implementations
- `ConfigPolicyPlugin` - Policy rule evaluation
- `MemoryServicePlugin` - Memory API client
- `StandardGatekeeperPlugin` - Quality gate evaluation

### Execution Flow
```
main.rs -> app.rs -> flow_standard.rs or flow_tui.rs
  -> plugins/plan.rs (build_runner_spec)
  -> core/engine/run.rs (run_with_query)
    -> pre.rs (memory search + inject)
    -> run.rs (backend execution)
    -> post.rs (gatekeeper + extract)
```

## Architecture Documentation

Detailed architecture analysis and design documentation:

### Core Documentation
- **[Architecture Analysis](docs/ARCHITECTURE_ANALYSIS.md)** - Comprehensive analysis of architecture issues and improvement proposals
  - MemoryPlugin trait definition/implementation coupling
  - Gatekeeper responsibility confusion
  - Memory concept dual implementation paths
  - Candidate/Hit/Validation lifecycle documentation

- **[Memory Architecture](docs/MEMORY_ARCHITECTURE.md)** - Memory system design and layer responsibilities
  - Design principles and current architecture
  - Layer responsibilities (Core vs Plugins)
  - Data flow (Search and Record workflows)
  - Extension guide (adding new Memory implementations)

- **[Terminology](docs/TERMINOLOGY.md)** - run_id vs session_id terminology definitions

### Key Architectural Insights

**Memory System**:
- **Core Layer** (`core/src/memory/`): Defines abstractions (trait, models, utilities)
- **Plugins Layer** (`plugins/src/memory/`): Implements MemoryPlugin trait
- **Known Issue**: MemoryClient (HTTP implementation) currently in core, should move to plugins

**Gatekeeper Responsibilities**:
- `prepare_inject()`: Pre-run QA item selection for prompt injection
- `evaluate()`: Post-run quality assessment, validation plans, and candidate decisions
- **Improvement Opportunity**: Further split evaluate responsibilities into quality assessment vs memory write decisions

**Memory Lifecycle**:
1. **Candidate** (validation_level=0): Newly extracted answer from execution
2. **Verified** (level=1): Validated through successful execution
3. **Confirmed** (level=2): Multiple successful validations
4. **Gold Standard** (level=3): High-frequency use with consistent success

See detailed lifecycle diagrams and decision logic in `docs/ARCHITECTURE_ANALYSIS.md` section 4.

## Configuration

Config loading priority (highest to lowest):
1. `~/.memex/config.toml`
2. `./config.toml` (current directory)
3. Built-in defaults

Key config sections: `control`, `logging`, `policy`, `memory`, `prompt_inject`, `gatekeeper`, `candidate_extract`, `events_out`, `tui`

## Coding Conventions

- **Formatter:** rustfmt (max_width=100)
- **Linter:** clippy with `-D warnings` (strict)
- **Allowed lints:** `too_many_arguments`, `module_inception`
- **Error handling:** Two-tier errors (`CliError` -> `RunnerError`)
- **Async:** tokio with process, io-util, macros, signal, rt-multi-thread, fs features
- **Testing:** Trait-based design for easy mocking; uses tokio-test, tempfile, mockito

## Dependencies

Key crates: tokio, clap (derive), serde/serde_json, tracing, reqwest, ratatui/crossterm, thiserror, chrono, uuid, toml

## CI/CD

- **ci.yml:** Runs on main/master/develop + PRs; lint job (fmt+clippy), test job (ubuntu + windows)
- **release.yml:** Triggered by `v*` tags; cross-platform builds (macOS ARM/Intel, Linux, Windows)

## Common Tasks

### Adding a new backend
1. Implement `BackendStrategy` trait in `plugins/src/backend/`
2. Add factory function in `plugins/src/factory.rs`
3. Update `BackendKind` enum in `cli/src/commands/cli.rs`

### Adding a new command
1. Add command struct in `cli/src/commands/cli.rs`
2. Add command to `Commands` enum in `cli/src/commands/cli.rs`
3. Add dispatch case in `cli/src/main.rs`
4. Implement handler in `cli/src/app.rs` or new module (e.g., `cli/src/commands/memory.rs` for memory service commands)

### Modifying configuration
1. Update types in `core/src/config/types.rs`
2. Update `config.toml` example
3. Update loading logic in `core/src/config/load.rs` if needed
