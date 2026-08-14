# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Nog is a project to create the full utility of the gitup ui (https://gitup.co/) using a colorized terminal interface.

The project should be built in whatever language has the bests text ui (tui) support for unix terminals.

## Tech Stack

- **Language:** Rust
- **TUI Framework:** Ratatui 0.29 + Crossterm 0.28
- **Git Access:** git2 0.19 (libgit2 bindings — direct database access, no shelling out)
- **Error handling:** anyhow

## Architecture

- `src/main.rs` — Entry point, terminal setup/teardown, event loop
- `src/app.rs` — Application state machine, key handling
- `src/ui.rs` — Top-level layout (graph pane + quick view pane)
- `src/theme.rs` — Color palette
- `src/git/` — Git data layer (types, repo, log walking, diff, status, stash)
- `src/graph/` — DAG layout algorithm, box-drawing glyphs
- `src/widgets/` — Ratatui widgets (graph view, commit detail, commit window, stash view, branch picker)
- `tests/` — Integration tests

## Build & Run

The toolchain is pinned to Rust 1.93.1 by `rust-toolchain.toml` (rustup) and
`.tool-versions` (asdf). Bump both together — they are read by different tools.

```bash
cargo build           # debug build
cargo run             # run in current git repo
cargo run -- /path    # run against specific repo
```

A Makefile wraps the common tasks. `make help` lists every target.

```bash
make install          # build --release and install to ~/bin (PREFIX/BINDIR override)
make test             # run the test suite
make fmt-check        # fail if sources are not formatted
make lint             # clippy with warnings denied
```

`make fmt-check` and `make lint` must pass before a change is committed.

## Key Bindings

- `j/k` or `↑/↓` — Navigate commits
- `Space` — Toggle Quick View (commit detail + diff)
- `/` — Search forward (type query, Enter to confirm, Esc to cancel)
- `?` — Search backward
- `n` — Next match
- `N` — Previous match
- `c` — Open commit window (stage/unstage files, write message, commit)
- `s` — Open stash view (browse stash stack, view files, apply/pop/drop)
- `b` — Open branch picker (Enter to checkout, Esc to cancel)
- `B` — Toggle branch filter (show only commits reachable from HEAD)
- `r` — Toggle remote branch tips
- `g/G` — Jump to first/last commit
- `Ctrl+d/u` — Page down/up
- `q/Esc` — Quit (or close Quick View)

