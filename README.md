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
- Commit window for staging files and writing commit messages
- Stash view for browsing, applying, popping, and dropping stashes
- Sync with a remote (stash, fetch, pop) and a conflict view to resolve merges
- Branch picker with checkout (stashes uncommitted changes when needed)
- Branch filter that hides everything not reachable from HEAD
- Local and remote branch/tag decorations on the graph
- Incremental search across commit hashes, summaries, authors, branches,
  and tags
- Auto-refresh via filesystem watcher (no manual reload required)
- Handles repositories with up to 50,000 commits


## Getting Started

### Requirements

- A Unix terminal with 256-color support
- Rust 1.93.1, pinned by `rust-toolchain.toml`

You do not need to install Rust by hand.  If you use `rustup`, the pinned
toolchain is fetched automatically on the first build.  If you use `asdf`,
`.tool-versions` records the same version:

```bash
asdf plugin add rust https://github.com/asdf-community/asdf-rust.git
asdf install
```

Note that these two files are read by different tools and nothing keeps them
in sync, so bump them together.

### Install

```bash
git clone https://github.com/r0llingp1n/nog.git
cd nog
make install
```

`make install` builds an optimized binary and installs it to `~/bin`.  If that
directory is not on your `PATH`, the install step says so.  Override the
destination with `PREFIX` or `BINDIR`:

```bash
make install BINDIR=/usr/local/bin
```

`make uninstall` removes it again.

### Run

```bash
nog                  # open the current directory's repository
nog /path/to/repo    # open a specific repository
nog --version        # print the version
```

To run without installing anything:

```bash
cargo run            # current directory
cargo run -- /path   # specific repository
```

### Development

```bash
make help            # list every target
make build           # debug build
make release         # optimized build
make test            # run the test suite
make fmt             # format sources in place
make fmt-check       # fail if sources are not formatted
make lint            # clippy with warnings denied
make check-versions  # verify the two toolchain pins agree
```

### Pre-commit hooks

The repository ships a [pre-commit](https://pre-commit.com/) configuration.
Install the hook once after cloning:

```bash
make init-pre-commit-hooks
```

Every `git commit` then checks formatting, runs clippy with warnings denied,
tidies whitespace, and verifies that `rust-toolchain.toml` and
`.tool-versions` still pin the same Rust version.  To run everything against
the whole tree without committing:

```bash
make pre-commit-run-all-files
```

The toolchain check is also available on its own as `make check-versions`, or
directly as `scripts/check-toolchain-versions.sh`.


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
| c                 | Open commit window   |
| s                 | Open stash view      |
| p                 | Pull                 |
| P                 | Pull (fast-forward)  |
| r                 | Toggle remote refs   |
| b                 | Open branch picker   |
| B                 | Toggle branch filter |

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

### Sync

`p` and `P` sync the current branch with its remote while keeping local
changes.  Both stash uncommitted changes, run `git fetch --all --prune`, then
pop the stash back onto the current branch:

- `p` — pull with a two-parent merge commit when the branch has diverged.
- `P` — pull as a fast-forward only.

If the pop or merge leaves a conflict, Nog opens a conflict view listing the
conflicted files.  Resolve them in your editor or the working tree, then press
`r` to re-check, `c` to commit the resolution, or `Esc` to abort and restore
the previous state.


## Architecture

```
src/
  main.rs          Entry point, terminal setup, event loop
  app.rs           Application state, key handling
  ui.rs            Top-level layout (graph pane + quick view pane)
  theme.rs         Color palette
  git/             Git data layer (types, repo access, log walking, diff,
                   status, stash, sync, fetch)
  graph/           DAG layout algorithm, box-drawing glyphs
  widgets/         Ratatui widgets (graph view, commit detail, commit
                   window, stash view, branch picker, conflict view)
tests/             Integration tests
scripts/           Repo maintenance scripts (pre-commit, Makefile)
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
