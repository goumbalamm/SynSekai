# SynSekai

A fast, keyboard-driven terminal torrent client built with Rust.

![Rust](https://img.shields.io/badge/rust-2024-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
[![CI](https://github.com/goumbalamm/SynSekai/actions/workflows/ci.yml/badge.svg)](https://github.com/goumbalamm/SynSekai/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/goumbalamm/SynSekai/branch/main/graph/badge.svg)](https://codecov.io/gh/goumbalamm/SynSekai)

## Features

- Full-screen TUI with live progress bars, download speed, and peer counts
- Add torrents from file paths, magnet links, drag-and-drop, or clipboard paste
- Readline-style input editing — feel at home if you use a shell
- Pause, resume, and remove torrents (with optional file deletion)
- Private tracker support (preserves `announce_sig`/`announce_ts` query params)
- Persists session across restarts via librqbit fast-resume

## Keybindings

### Normal mode

| Key | Action |
|-----|--------|
| `a` | Open add-torrent dialog |
| `p` | Pause / resume selected torrent |
| `d` | Open remove-torrent dialog |
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `q` / `Ctrl+C` | Quit |

### Add-torrent dialog

| Key | Action |
|-----|--------|
| `Enter` | Confirm |
| `Esc` | Cancel |
| `←` / `→` | Move cursor |
| `Home` / `Ctrl+A` | Jump to start |
| `End` / `Ctrl+E` | Jump to end |
| `Ctrl+←` / `Ctrl+→` | Move by word |
| `Backspace` | Delete char back |
| `Ctrl+Backspace` / `Ctrl+W` | Delete word back |
| `Ctrl+Delete` | Delete word forward |
| `Ctrl+K` | Delete to end of line |
| `Ctrl+U` | Delete to start of line |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+C` | Copy input to clipboard |

Drag-and-drop a `.torrent` file directly into the dialog — paths wrapped in quotes are stripped automatically.

### Remove dialog

| Key | Action |
|-----|--------|
| `Space` | Toggle "delete files" |
| `Enter` | Confirm removal |
| `Esc` | Cancel |

## Installation

### Prerequisites

- Rust toolchain (1.85+ for edition 2024)

### Build from source

```bash
git clone https://github.com/goumbalamm/SynSekai
cd SynSekai
cargo build --release
./target/release/synsekai
```

### Options

```
Usage: synsekai [OPTIONS]

Options:
  --output-dir <DIR>  Directory to save downloaded files [default: ~/Downloads]
  -h, --help          Print help
```

## Development

```bash
# Run all tests
cargo test

# Hot-reload during development
cargo watch -x run

# Test coverage
cargo tarpaulin --config tarpaulin.toml
```

## Architecture

```
src/
├── main.rs          # CLI entry point (clap)
├── app.rs           # Pure UI state (no I/O)
├── engine.rs        # librqbit wrapper (all torrent I/O here)
├── terminal.rs      # Terminal setup / teardown
├── tui.rs           # Event loop + key handlers
├── types.rs         # Shared types (TorrentRow, AppMode, InputState)
└── ui/
    ├── mod.rs           # Top-level layout
    ├── torrent_table.rs # Torrent list widget
    ├── status_bar.rs    # Bottom hint bar
    └── popups.rs        # Add & confirm-remove dialogs

patches/
└── librqbit-tracker-comms/  # Local patch preserving private tracker query params
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| [librqbit](https://github.com/ikatson/rqbit) | BitTorrent engine |
| [ratatui](https://github.com/ratatui/ratatui) | TUI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Terminal backend |
| [tokio](https://tokio.rs) | Async runtime |
| [arboard](https://github.com/1Password/arboard) | Clipboard access |

## License

MIT
