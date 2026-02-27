# Contributing to SynSekai

Thanks for your interest in contributing!

---

## Before you start

- Open an issue first for non-trivial changes so we can align on the approach before you invest time writing code.
- For small bug fixes or typos, a PR is fine without an issue.

---

## Setup

```bash
git clone https://github.com/goumbalamm/SynSekai
cd SynSekai
cargo build
cargo test
```

Requirements: Rust 1.85+ (edition 2024).

---

## Workflow

1. Fork the repo and create a branch from `main`:
   ```bash
   git checkout -b fix/my-bug
   ```

2. Make your changes following the conventions below.

3. Make sure the full suite passes locally:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```

4. Open a pull request against `main`. The CI must be green before it can be merged.

---

## Conventions

### Tests first
This project is developed TDD. Every new behaviour needs a test. Write the failing test, then the implementation. See existing tests in each module for examples.

### No binary test fixtures
Generate torrent data programmatically in tests — do not commit `.torrent` or other binary files. Use the `write_minimal_torrent()` helper pattern already in the codebase.

### Keep layers separate
- `engine.rs` is the only file allowed to import librqbit types.
- `app.rs` holds pure UI state — no I/O, no async.
- Key handlers in `tui.rs` return `Option<Action>` and must remain pure functions.

### Code style
- No `unwrap()` in production code — use `?` and `anyhow`.
- No dead fields, unused imports, or clippy warnings.
- Do not add comments or docs to code you did not change.
- Do not leave debug prints in committed code.

### Commits
Follow conventional commits: `feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `ci:`.

---

## What's in scope

- Bug fixes
- Performance improvements
- New keybindings or UX improvements
- Better error messages
- Platform-specific fixes (Windows, Linux, macOS)

## What to discuss first

- New dependencies
- Changes to the engine/API layer
- Anything that touches the librqbit session lifecycle
