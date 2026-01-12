# Changelog

All notable changes to the memory plugin will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-01-12

### Added

#### Core Features
- **Automatic memory retrieval**: UserPromptSubmit hook searches memory and injects relevant context
- **Automatic knowledge recording**: PostToolUse hook captures Write/Edit/Bash tool usage
- **Session recording**: Stop/SubagentStop/PreCompact hooks record complete sessions
- **Session lifecycle management**: SessionStart/SessionEnd hooks for initialization and cleanup

#### Skills
- **memory-usage**: Comprehensive guide for using memory features
  - Search functionality guide
  - Recording functionality guide
  - Automatic hooks explanation
  - memex-cli command reference
  - Configuration guide
  - Common workflows
- **hook-troubleshooting**: Debug and troubleshooting guide
  - Common issues and solutions
  - Log analysis techniques
  - Diagnostic procedures
  - Error catalog reference

#### Commands
- **/setup-memex**: Interactive configuration wizard
  - Guided setup for `.claude/memory.local.md`
  - Service URL configuration
  - API key setup
  - Search parameter tuning
  - memex-cli path configuration
- **/test-memory**: Comprehensive connectivity testing
  - memex-cli availability check
  - Memory service health check
  - Search functionality test
  - Record functionality test
  - Python dependencies validation
  - Hook scripts validation
- **/validate-hooks**: Configuration validation
  - hooks.json structure validation
  - Script reference validation
  - Python syntax validation
  - Dependencies validation
  - Portable path reference validation
  - Timeout configuration validation
  - Settings file validation
- **/view-memory-logs**: Log viewer and analyzer
  - Recent logs display
  - Error filtering
  - Activity summary
  - Log statistics
  - Search functionality

#### Hook Scripts
- `scripts/session-init.py`: Initialize session state and environment
- `scripts/session-cleanup.py`: Clean up session resources
- `scripts/memory-inject.py`: Search and inject memory context
- `scripts/memory-record.py`: Record tool usage as knowledge
- `scripts/memory-hit.py`: Record memory hits
- `scripts/record-session-enhanced.py`: Enhanced session recording
- `scripts/http_client.py`: HTTP client for memory service
- `scripts/gatekeeper.py`: Quality gate for memory persistence
- `scripts/server_manager.py`: Memory service management
- `scripts/project_utils.py`: Project utilities
- `scripts/transcript_parser.py`: Parse conversation transcripts
- `scripts/long_text_handler.py`: Handle long text content

#### Configuration
- Project-specific settings via `.claude/memory.local.md`
- Global configuration support via `~/.memex/config.toml`
- Configurable search parameters (limit, min_score)
- Custom memex-cli path support
- Enable/disable toggle for hooks

#### Documentation
- Comprehensive README with installation and usage guide
- Settings template with detailed field explanations
- Reference docs for search parameters optimization
- Reference docs for recording strategies
- Example queries and common patterns
- Common errors catalog

#### Infrastructure
- Standard Claude Code plugin structure
- Portable path references using ${CLAUDE_PLUGIN_ROOT}
- Python requirements management (requirements-http.txt)
- Cross-platform support (Linux, macOS, Windows)
- .gitignore for user-specific settings

### Dependencies
- Python 3.8+
- requests library (HTTP client)
- memex-cli (memory service CLI tool)
- Access to memex memory service

### Plugin Metadata
- **Name**: memory
- **Version**: 1.0.0
- **Author**: Memex CLI Team
- **License**: MIT
- **Keywords**: memory, context, retrieval, memex, session-management

### Notes
- This is the initial release
- Hooks require Claude Code restart to load configuration changes
- Settings in `.claude/memory.local.md` override global `~/.memex/config.toml`
- Designed for public release on Claude Marketplace

### Migration Guide
For users migrating from manual hook setup:
1. Back up existing `.claude/settings.json` hooks configuration
2. Install memory plugin
3. Run `/setup-memex` to create project settings
4. Remove manual hooks from `.claude/settings.json`
5. Restart Claude Code

### Known Limitations
- Hooks cannot be hot-swapped (require restart)
- Windows may require Python path adjustments
- Large search limits (>20) may impact performance
- Log files grow unbounded (manual cleanup recommended)

---

## Future Releases

### Planned for 1.1.0
- [ ] Automatic log rotation
- [ ] Settings UI in Claude Code
- [ ] Performance metrics dashboard
- [ ] Memory service auto-discovery
- [ ] Advanced filtering options

### Planned for 1.2.0
- [ ] Multi-service support
- [ ] Caching layer for frequent queries
- [ ] Batch recording optimization
- [ ] Export/import memory archives

### Planned for 2.0.0
- [ ] Prompt-based hooks for intelligent gating
- [ ] Machine learning for relevance scoring
- [ ] Distributed memory support
- [ ] Real-time memory sync

---

## Version History

| Version | Date | Description |
|---------|------|-------------|
| 1.0.0 | 2026-01-12 | Initial release with core memory features |

---

For detailed upgrade instructions and breaking changes, see [UPGRADE.md](UPGRADE.md) (to be created in future releases).

For contributing guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md) (to be created).
