```markdown
## [4.0.0] - 2026-02-02

### Added
* **Multi‑Provider LLM Support** – Ganesha CLI can now interface with both local and cloud language models, allowing users to choose the best provider for their workflow.
* **MCP Tool Integration** – Seamless integration with the MCP (Model Control Plane) tool for advanced model management and deployment.
* **TUI Mode** – A fully‑featured Text User Interface is available via `ganesha tui`, providing an interactive experience in terminal environments.
* **Voice Input** – Users can now speak commands or prompts directly into Ganesha using the new voice input module (`ganesha voice`).
* **Vision Capture** – The CLI can capture images from a webcam or upload image files for vision‑enabled LLMs (`ganesha vision`).
* **Rollback System** – A built‑in rollback mechanism lets users revert to previous model states or configurations with `ganesha rollback`.

### Changed
* Updated the configuration schema to support provider selection, TUI settings, and voice/vision options.
* Refactored command parsing to accommodate the new subcommands (`tui`, `voice`, `vision`, `rollback`).
* Improved error handling for multi‑provider connections.

### Fixed
* Resolved a crash when switching providers mid‑session.
* Fixed path
