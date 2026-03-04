use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use git2::Oid;

use crate::git::diff::{self, DiffData};
use crate::git::log;
use crate::git::repo;
use crate::git::types::{CommitInfo, Decorations};
use crate::graph::layout;
use crate::graph::types::GraphRow;
use crate::widgets::commit_detail::CommitDetailState;
use crate::widgets::graph_view::GraphViewState;

const MAX_COMMITS: usize = 50_000;

#[derive(PartialEq)]
pub enum AppMode {
    GraphView,
    QuickView,
}

#[allow(dead_code)]
pub struct App {
    pub mode: AppMode,
    pub should_quit: bool,
    pub repo_path: PathBuf,

    pub commits: Vec<CommitInfo>,
    pub graph_rows: Vec<GraphRow>,
    pub decorations: HashMap<Oid, Decorations>,

    pub graph_state: GraphViewState,
    pub detail_state: CommitDetailState,

    // Cached data for Quick View
    pub selected_message: Option<String>,
    pub selected_diff: Option<DiffData>,

    // For repo access when loading diffs
    repo_internal_path: PathBuf,
    pub branch_name: String,
}

impl App {
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        let repository = repo::open_repo(&repo_path)?;
        let repo_internal_path = repository.path().to_path_buf();
        let branch_name = repository
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(String::from))
            .unwrap_or_else(|| "HEAD (detached)".to_string());
        let commits = log::walk_commits(&repository, MAX_COMMITS)?;
        let decorations = repo::load_decorations(&repository)?;
        let graph_rows = layout::compute_graph_layout(&commits);

        Ok(Self {
            mode: AppMode::GraphView,
            should_quit: false,
            repo_path,
            commits,
            graph_rows,
            decorations,
            graph_state: GraphViewState::new(),
            detail_state: CommitDetailState::new(),
            selected_message: None,
            selected_diff: None,
            repo_internal_path,
            branch_name,
        })
    }

    pub fn current_branch_name(&self) -> &str {
        &self.branch_name
    }

    pub fn selected_commit(&self) -> Option<&CommitInfo> {
        self.commits.get(self.graph_state.selected)
    }

    fn load_quick_view(&mut self) -> Result<()> {
        if let Some(commit) = self.selected_commit().cloned() {
            let repo = git2::Repository::open(&self.repo_internal_path)?;
            self.selected_message = Some(diff::get_commit_message(&repo, commit.oid)?);
            self.selected_diff = Some(diff::diff_commit(&repo, commit.oid)?);
            self.detail_state.scroll = 0;
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            AppMode::GraphView => self.handle_graph_key(key),
            AppMode::QuickView => self.handle_quick_view_key(key),
        }
    }

    fn handle_graph_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.graph_state.selected + 1 < self.commits.len() {
                    self.graph_state.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.graph_state.selected = self.graph_state.selected.saturating_sub(1);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.graph_state.selected = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                if !self.commits.is_empty() {
                    self.graph_state.selected = self.commits.len() - 1;
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.graph_state.selected =
                    (self.graph_state.selected + 20).min(self.commits.len().saturating_sub(1));
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.graph_state.selected = self.graph_state.selected.saturating_sub(20);
            }
            KeyCode::Char(' ') => {
                let _ = self.load_quick_view();
                self.mode = AppMode::QuickView;
            }
            _ => {}
        }
    }

    fn handle_quick_view_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char(' ') => {
                self.mode = AppMode::GraphView;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.detail_state.scroll = self.detail_state.scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.detail_state.scroll = self.detail_state.scroll.saturating_sub(1);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.detail_state.scroll = self.detail_state.scroll.saturating_add(20);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.detail_state.scroll = self.detail_state.scroll.saturating_sub(20);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.detail_state.scroll = 0;
            }
            _ => {}
        }
    }
}
