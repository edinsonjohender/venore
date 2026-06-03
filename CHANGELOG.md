# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Public release scaffolding: `SECURITY.md`, `.github/` issue and PR templates,
  `CHANGELOG.md`, `CODE_OF_CONDUCT.md`, `.editorconfig`, `rustfmt.toml`,
  `clippy.toml`.
- `package.json` (UI) now declares `description`, `homepage`, `license`,
  `repository`, and `bugs`.
- `tauri.conf.json` ships with a baseline Content Security Policy
  (replacing the previous `null`).
- Idempotent prompt-registry migration that renames the
  `chat-fragment-bitacoras-hint` row to `chat-fragment-logbook-hint`.

### Changed
- Full Spanish → English translation across the codebase: comments,
  doc-comments, LLM prompts, tool descriptions, tool output, hardcoded UI
  strings, and per-module README files.
- `BITACORAS_HINT` constant renamed to `LOGBOOK_HINT`.
- `authors` field set to `Edinson Johender <hi@skalar.app>` across crates.
- Internal architecture / strategy notes under `docs/` are no longer
  tracked (added to `.gitignore` — kept locally by the maintainer).

### Fixed
- Five pre-existing test failures:
  - `analysis::project_analyzer` detection tests no longer rely on the
    shared system temp dir.
  - `tools::executor` read-logbook assertion updated to match the current
    section header format.
  - `tools::executor` propose-logbook-write tests updated for the
    pending-writes approval flow.

### Removed
- `Cargo.lock` and `pnpm-lock.yaml` are no longer git-ignored (binaries
  and apps should track them for reproducible builds).
- `crates/wizard_debug.txt` (developer-machine debug log, accidentally
  committed earlier).
