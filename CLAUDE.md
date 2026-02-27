# CLAUDE.md — SynSekai

Instructions for Claude Code when working in this repository.

---

## Commits

- **Never add `Co-Authored-By: Claude` lines** to commit messages.
- Commits are authored solely by the repository owner.
- Follow conventional commits: `feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `ci:`.
- Do not push without being asked.
- Do not force-push `main` without being asked.

---

## Project

Standalone Rust TUI torrent client.

- **Binary**: `synsekai` (`src/main.rs`)
- **Stack**: Rust 2024 · librqbit 8 · ratatui 0.29 · crossterm 0.28 · tokio
- **Build**: `cargo build --release`
- **Run**: `cargo run -- --output-dir /tmp/dl`
- **Test**: `cargo test`
- **Hot reload**: `cargo watch -x run`
- **Coverage (local)**: `cargo tarpaulin --config tarpaulin.toml`

---

## Source layout

```
src/
├── main.rs          # CLI entry point (clap) — excluded from coverage
├── app.rs           # Pure UI state — no I/O, fully unit-testable
├── engine.rs        # librqbit wrapper — all torrent I/O lives here
├── terminal.rs      # Terminal setup / teardown (alternate screen, raw mode, panic hook) — excluded from coverage
├── tui.rs           # Async event loop + pure key handlers (key_normal, key_add_dialog, …)
├── types.rs         # Shared types: TorrentRow, AppMode, InputState
└── ui/
    ├── mod.rs           # Top-level layout (table + status bar)
    ├── torrent_table.rs # Torrent list widget
    ├── status_bar.rs    # Bottom keybinding hint bar
    └── popups.rs        # Add-torrent and confirm-remove dialogs

patches/
└── librqbit-tracker-comms/   # Local patch — preserves announce_sig/announce_ts query params
                               # Wired via [patch.crates-io] in Cargo.toml

.github/
└── workflows/
    └── ci.yml            # CI: fmt, clippy, build, test, coverage → Codecov
```

---

## Architecture rules

- `engine.rs` is the **only** file that imports librqbit types. No other module touches librqbit directly.
- `app.rs` holds pure UI state — never calls async code or engine methods. `#[derive(Default)]` on `App`.
- `tui.rs` key handlers return `Option<Action>` — pure functions testable without a terminal.
- Never hold the app state lock across an `.await` on the engine.

---

## librqbit patterns

```rust
// Session creation (production)
SessionOptions {
    disable_dht: false,
    disable_dht_persistence: false,
    fastresume: true,
    persistence: Some(SessionPersistenceConfig::Json { folder: None }),
    listen_port_range: Some(6881..6891),   // required — omitting → port=0 in tracker announce
    enable_upnp_port_forwarding: true,
    peer_id: Some(generate_azereus_style(*b"qB", (4, 6, 0, 0))),
    ..Default::default()
}

// Tests: disable DHT to avoid port conflicts with a running instance
SessionOptions { disable_dht: true, disable_dht_persistence: true, ..Default::default() }

// Adding a torrent (handles magnet links, URLs, and local paths)
AddTorrent::from_cli_argument(input)

// Pause / resume
api.api_torrent_action_pause(TorrentIdOrHash::Id(id))
api.api_torrent_action_start(TorrentIdOrHash::Id(id))   // "start" = resume

// Remove (keep files) / remove (delete files)
api.api_torrent_action_forget(TorrentIdOrHash::Id(id))
api.api_torrent_action_delete(TorrentIdOrHash::Id(id))
```

- `Session::new_with_opts` returns `Arc<Session>` — do **not** wrap in `Arc::new()` again.
- `features = ["default-tls"]` is required on librqbit when `default-features = false`.
- After adding a torrent the status is `Initializing` briefly — poll with 100 ms sleeps before pausing in tests.

---

## Tracker patch

`librqbit-tracker-comms 3.0.0` used `tracker_url.set_query(…)` which replaced the entire query string, stripping `announce_sig`/`announce_ts` from private tracker URLs (Ygg, etc.).

The local patch at `patches/librqbit-tracker-comms/` preserves existing query params and appends standard tracker params after them. Wired via `[patch.crates-io]` in `Cargo.toml`.

---

## Testing conventions

- **TDD**: write a failing test first, then implement.
- Every public behaviour has a unit test. UI modules use `ratatui::backend::TestBackend`.
- **No binary test fixtures committed to the repo.** Generate torrent data programmatically:

```rust
fn minimal_torrent_bytes() -> Vec<u8> {
    // Minimal valid single-file torrent in bencode:
    // { "info": { "length": 1, "name": "t", "piece length": 16384, "pieces": <20×0x00> } }
    let mut v = Vec::new();
    v.extend_from_slice(b"d4:infod6:lengthi1e4:name1:t12:piece lengthi16384e6:pieces20:");
    v.extend_from_slice(&[0u8; 20]);
    v.extend_from_slice(b"ee");
    v
}

fn write_minimal_torrent() -> tempfile::NamedTempFile {
    use std::io::Write;
    let mut f = tempfile::Builder::new().suffix(".torrent").tempfile().unwrap();
    f.write_all(&minimal_torrent_bytes()).unwrap();
    f  // keep alive for the duration of the test — drops = deletes the file
}
```

- The one ignored test (`real_dht_finds_peers_for_fixture_torrent`) requires a real `.torrent` file and network; run manually with `cargo test -- --ignored --nocapture`.

---

## CI / GitHub

- **Workflow**: `.github/workflows/ci.yml` — runs on push and PR to `main`.
  - Steps: `cargo fmt --check` → `cargo clippy -D warnings` → `cargo build --release` → `cargo test` → tarpaulin coverage → Codecov upload.
  - Uses `taiki-e/install-action` to install `cargo-tarpaulin` in CI.
  - Codecov token stored as `CODECOV_TOKEN` GitHub secret.
- **Branch protection on `main`**:
  - All changes via PR — no direct pushes.
  - CI (`Build & Test`) must pass before merge.
  - 1 approving review required; stale reviews dismissed on new commits.
  - Force pushes blocked.
- **Coverage exclusions** (in `tarpaulin.toml`): `src/main.rs`, `src/terminal.rs`.

---

## Code style

- Rust 2024 edition.
- No `unwrap()` in production code — use `?` and `anyhow`.
- No dead fields or unused imports — fix warnings before committing.
- Do not add docstrings, comments, or type annotations to code that wasn't changed.
- Do not leave debug panels, `eprintln!`, or temporary logging in committed code.
- Keep UI state (`app.rs`) and I/O (`engine.rs`) strictly separated.
- Clippy is run with `-D warnings` — zero tolerance for warnings.

---

## Input editing (add-dialog)

Full readline-style shortcuts implemented in `key_add_dialog` (`tui.rs`) backed by `InputState` methods (`types.rs`):

| Key | Action |
|-----|--------|
| `←` / `→` | Move char |
| `Ctrl+←` / `Ctrl+→` | Move word |
| `Home` / `Ctrl+A` | Start of line |
| `End` / `Ctrl+E` | End of line |
| `Backspace` | Delete char back |
| `Ctrl+Backspace` / `Ctrl+W` | Delete word back |
| `Ctrl+Delete` | Delete word forward |
| `Ctrl+K` | Delete to end |
| `Ctrl+U` | Delete to start |
| `Ctrl+V` | Paste (macOS: tries osascript for file refs first, falls back to arboard) |
| `Ctrl+C` | Copy to clipboard |
| `Enter` | Submit (strips surrounding quotes) |
| `Esc` | Cancel |
