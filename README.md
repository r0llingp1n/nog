# Nog

Nog is a terminal-based Git client inspired by [GitUp](https://gitup.co/).
It renders a colorized commit graph with branch decoration, commit details,
and inline diffs -- all without leaving the terminal.

Nog reads the repository directly through libgit2, so it never shells out to
`git`.  It watches the `.git` directory for changes and automatically refreshes
the view when commits, refs, or the index change.


## Features

- Commit graph with box-drawing glyphs and per-branch coloring
- Quick View panel showing full commit messages and unified diffs
- Branch picker with checkout (stashes uncommitted changes when needed)
- Local and remote branch/tag decorations on the graph
- Incremental search across commit hashes, summaries, authors, branches,
  and tags
- Auto-refresh via filesystem watcher (no manual reload required)
- Handles repositories with up to 50,000 commits


## Requirements

- Rust 1.70 or later
- A Unix terminal with 256-color support


## Building

```
cargo build            # debug build
cargo build --release  # optimized build
```


## Usage

```
cargo run              # open the current directory's repository
cargo run -- /path     # open a specific repository
```

Or, after building a release binary:

```
./target/release/nog          # current directory
./target/release/nog /path    # specific repository
```


## Key Bindings

### Navigation

| Key               | Action               |
|-------------------|----------------------|
| j / Down          | Next commit          |
| k / Up            | Previous commit      |
| g / Home          | First commit         |
| G / End           | Last commit          |
| Ctrl-d            | Page down            |
| Ctrl-u            | Page up              |

### Views

| Key               | Action               |
|-------------------|----------------------|
| Space             | Toggle Quick View    |
| r                 | Toggle remote refs   |
| b                 | Open branch picker   |

### Search

| Key               | Action               |
|-------------------|----------------------|
| /                 | Search forward       |
| ?                 | Search backward      |
| n                 | Next match           |
| N                 | Previous match       |

### General

| Key               | Action               |
|-------------------|----------------------|
| q / Esc           | Quit (or close view) |


## Architecture

```
src/
  main.rs          Entry point, terminal setup, event loop
  app.rs           Application state, key handling
  ui.rs            Top-level layout (graph pane + quick view pane)
  theme.rs         Color palette
  git/             Git data layer (types, repo access, log walking, diff)
  graph/           DAG layout algorithm, box-drawing glyphs
  widgets/         Ratatui widgets (graph view, commit detail)
```

### Dependencies

- **Ratatui** + **Crossterm** -- terminal UI rendering
- **git2** -- libgit2 bindings for direct repository access
- **notify** -- filesystem watching for auto-refresh
- **anyhow** -- error handling
- **chrono** -- timestamp formatting


## License

This project is licensed under the GNU General Public License v3.0.
See [LICENSE](LICENSE) for the full text.
