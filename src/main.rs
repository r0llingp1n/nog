mod app;
mod git;
mod graph;
mod theme;
mod ui;
mod widgets;

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use notify::{RecursiveMode, Watcher};
use ratatui::prelude::*;

fn main() -> Result<()> {
    let repo_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let mut app = app::App::new(repo_path)?;

    // Set up file watcher on .git directory
    let (fs_tx, fs_rx) = mpsc::channel();
    let _watcher = setup_watcher(&app.repo_path, fs_tx);

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_loop(&mut terminal, &mut app, &fs_rx);

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn setup_watcher(
    repo_path: &PathBuf,
    tx: mpsc::Sender<()>,
) -> Option<notify::RecommendedWatcher> {
    // Watch the .git directory for changes (commits, refs, index, etc.)
    let git_dir = if let Ok(repo) = git::repo::open_repo(repo_path) {
        repo.path().to_path_buf()
    } else {
        return None;
    };

    // Also watch the workdir for index/staging changes
    let workdir = if let Ok(repo) = git::repo::open_repo(repo_path) {
        repo.workdir().map(|p| p.to_path_buf())
    } else {
        None
    };

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            // Only trigger on modify/create/remove, skip access events
            use notify::EventKind;
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    let _ = tx.send(());
                }
                _ => {}
            }
        }
    })
    .ok()?;

    // Watch .git (refs, HEAD, index changes)
    let _ = watcher.watch(&git_dir, RecursiveMode::Recursive);

    // Watch workdir non-recursively for top-level changes
    if let Some(wd) = workdir {
        let _ = watcher.watch(&wd, RecursiveMode::NonRecursive);
    }

    Some(watcher)
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut app::App,
    fs_rx: &mpsc::Receiver<()>,
) -> Result<()> {
    // Debounce: don't reload more than once per 500ms
    let mut last_reload = Instant::now();
    let debounce = Duration::from_millis(500);

    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }

        // Check for filesystem changes (non-blocking drain)
        let mut needs_reload = false;
        while fs_rx.try_recv().is_ok() {
            needs_reload = true;
        }

        if needs_reload && last_reload.elapsed() >= debounce {
            app.refresh();
            last_reload = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
