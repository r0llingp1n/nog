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
    BranchPicker,
    StashPrompt,
    Search,
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
    pub show_remotes: bool,

    // Branch picker state
    pub branches: Vec<String>,
    pub branch_selected: usize,
    pub branch_error: Option<String>,

    // Stash prompt state
    pub stash_input: String,
    pub stash_target_branch: Option<String>,
    pub stash_error: Option<String>,

    // Search state
    pub search_input: String,
    pub search_pattern: Option<String>,
    pub search_matches: Vec<usize>,
    pub search_match_idx: Option<usize>,
    pub search_direction: SearchDirection,
}

#[derive(PartialEq, Clone, Copy)]
pub enum SearchDirection {
    Forward,
    Backward,
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
            show_remotes: false,
            branches: Vec::new(),
            branch_selected: 0,
            branch_error: None,
            stash_input: String::new(),
            stash_target_branch: None,
            stash_error: None,
            search_input: String::new(),
            search_pattern: None,
            search_matches: Vec::new(),
            search_match_idx: None,
            search_direction: SearchDirection::Forward,
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
            AppMode::BranchPicker => self.handle_branch_picker_key(key),
            AppMode::StashPrompt => self.handle_stash_prompt_key(key),
            AppMode::Search => self.handle_search_key(key),
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
            KeyCode::Char('r') => {
                self.show_remotes = !self.show_remotes;
            }
            KeyCode::Char('b') => {
                self.open_branch_picker();
            }
            KeyCode::Char('/') => {
                self.search_input.clear();
                self.search_direction = SearchDirection::Forward;
                self.mode = AppMode::Search;
            }
            KeyCode::Char('?') => {
                self.search_input.clear();
                self.search_direction = SearchDirection::Backward;
                self.mode = AppMode::Search;
            }
            KeyCode::Char('n') => {
                self.search_next();
            }
            KeyCode::Char('N') => {
                self.search_prev();
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

    fn open_branch_picker(&mut self) {
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            let mut branches = Vec::new();
            if let Ok(iter) = repo.branches(Some(git2::BranchType::Local)) {
                for branch_result in iter {
                    if let Ok((branch, _)) = branch_result {
                        if let Ok(Some(name)) = branch.name() {
                            branches.push(name.to_string());
                        }
                    }
                }
            }
            branches.sort();
            // Pre-select current branch
            let selected = branches
                .iter()
                .position(|b| b == &self.branch_name)
                .unwrap_or(0);
            self.branches = branches;
            self.branch_selected = selected;
            self.branch_error = None;
            self.mode = AppMode::BranchPicker;
        }
    }

    fn handle_branch_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.mode = AppMode::GraphView;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.branch_selected + 1 < self.branches.len() {
                    self.branch_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.branch_selected = self.branch_selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                self.checkout_selected_branch();
            }
            _ => {}
        }
    }

    fn checkout_selected_branch(&mut self) {
        let branch_name = match self.branches.get(self.branch_selected) {
            Some(name) => name.clone(),
            None => return,
        };

        if branch_name == self.branch_name {
            self.mode = AppMode::GraphView;
            return;
        }

        self.try_checkout(&branch_name);
    }

    fn try_checkout(&mut self, branch_name: &str) {
        let result = (|| -> Result<()> {
            let repo = git2::Repository::open(&self.repo_internal_path)?;
            let branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
            let refname = branch
                .get()
                .name()
                .ok_or_else(|| anyhow::anyhow!("invalid branch ref"))?;
            let obj = repo.revparse_single(refname)?;
            repo.checkout_tree(&obj, None)?;
            repo.set_head(refname)?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.branch_name = branch_name.to_string();
                let _ = self.reload();
                self.mode = AppMode::GraphView;
            }
            Err(e) => {
                let err_msg = e.to_string();
                // git2 conflict errors contain "conflict" or "checkout" + "overwritten"
                if err_msg.contains("conflict")
                    || err_msg.contains("overwritten")
                    || err_msg.contains("uncommitted")
                {
                    self.stash_target_branch = Some(branch_name.to_string());
                    self.stash_input.clear();
                    self.mode = AppMode::StashPrompt;
                } else {
                    self.branch_error = Some(format!("Checkout failed: {}", e));
                }
            }
        }
    }

    fn handle_stash_prompt_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.stash_target_branch = None;
                self.stash_input.clear();
                self.stash_error = None;
                self.mode = AppMode::BranchPicker;
            }
            KeyCode::Enter => {
                self.stash_and_checkout();
            }
            KeyCode::Backspace => {
                self.stash_input.pop();
            }
            KeyCode::Char(c) => {
                self.stash_input.push(c);
            }
            _ => {}
        }
    }

    fn stash_and_checkout(&mut self) {
        // Keep target_branch set so the prompt can redisplay on error
        let target = match self.stash_target_branch.as_ref() {
            Some(t) => t.clone(),
            None => return,
        };

        let stash_msg = if self.stash_input.is_empty() {
            None
        } else {
            Some(self.stash_input.clone())
        };

        let stash_result = (|| -> Result<()> {
            let mut repo = git2::Repository::open(&self.repo_internal_path)?;
            let sig = repo.signature()?;
            let msg = stash_msg.as_deref().unwrap_or("nog auto-stash");
            repo.stash_save(&sig, msg, None)?;
            Ok(())
        })();

        match stash_result {
            Ok(()) => {
                self.stash_input.clear();
                self.stash_target_branch = None;
                self.stash_error = None;
                self.try_checkout(&target);
            }
            Err(e) => {
                self.stash_error = Some(format!("Stash failed: {}", e));
                // Stay in StashPrompt so the user can see the error and retry
            }
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // Cancel search, clear pattern
                self.search_input.clear();
                self.search_matches.clear();
                self.search_match_idx = None;
                self.mode = AppMode::GraphView;
            }
            KeyCode::Enter => {
                // Confirm search
                self.execute_search();
                self.mode = AppMode::GraphView;
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                self.live_search();
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
                self.live_search();
            }
            _ => {}
        }
    }

    fn live_search(&mut self) {
        if self.search_input.is_empty() {
            self.search_matches.clear();
            self.search_match_idx = None;
            return;
        }
        self.execute_search();
    }

    fn execute_search(&mut self) {
        if self.search_input.is_empty() {
            self.search_pattern = None;
            self.search_matches.clear();
            self.search_match_idx = None;
            return;
        }

        let pattern = self.search_input.to_lowercase();
        self.search_pattern = Some(self.search_input.clone());

        // Match against hash, summary, author, and decorations
        self.search_matches = self
            .commits
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let haystack_parts = [
                    c.short_id.as_str(),
                    c.summary.as_str(),
                    c.author_name.as_str(),
                    &format!("{}", c.oid),
                ];
                if haystack_parts
                    .iter()
                    .any(|s| s.to_lowercase().contains(&pattern))
                {
                    return true;
                }
                // Check decorations
                if let Some(d) = self.decorations.get(&c.oid) {
                    for branch in &d.branches {
                        if branch.name.to_lowercase().contains(&pattern) {
                            return true;
                        }
                    }
                    for tag in &d.tags {
                        if tag.name.to_lowercase().contains(&pattern) {
                            return true;
                        }
                    }
                }
                false
            })
            .map(|(i, _)| i)
            .collect();

        // Jump to the nearest match from current position
        if !self.search_matches.is_empty() {
            let cur = self.graph_state.selected;
            if self.search_direction == SearchDirection::Forward {
                // Find first match at or after current position
                let idx = self
                    .search_matches
                    .iter()
                    .position(|&m| m >= cur)
                    .unwrap_or(0);
                self.search_match_idx = Some(idx);
                self.graph_state.selected = self.search_matches[idx];
            } else {
                // Find last match at or before current position
                let idx = self
                    .search_matches
                    .iter()
                    .rposition(|&m| m <= cur)
                    .unwrap_or(self.search_matches.len() - 1);
                self.search_match_idx = Some(idx);
                self.graph_state.selected = self.search_matches[idx];
            }
        } else {
            self.search_match_idx = None;
        }
    }

    fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let next = match self.search_match_idx {
            Some(idx) => {
                if self.search_direction == SearchDirection::Forward {
                    (idx + 1) % self.search_matches.len()
                } else {
                    if idx == 0 {
                        self.search_matches.len() - 1
                    } else {
                        idx - 1
                    }
                }
            }
            None => 0,
        };
        self.search_match_idx = Some(next);
        self.graph_state.selected = self.search_matches[next];
    }

    fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let prev = match self.search_match_idx {
            Some(idx) => {
                if self.search_direction == SearchDirection::Forward {
                    if idx == 0 {
                        self.search_matches.len() - 1
                    } else {
                        idx - 1
                    }
                } else {
                    (idx + 1) % self.search_matches.len()
                }
            }
            None => 0,
        };
        self.search_match_idx = Some(prev);
        self.graph_state.selected = self.search_matches[prev];
    }

    fn reload(&mut self) -> Result<()> {
        let repo = git2::Repository::open(&self.repo_internal_path)?;
        self.commits = log::walk_commits(&repo, MAX_COMMITS)?;
        self.decorations = repo::load_decorations(&repo)?;
        self.graph_rows = layout::compute_graph_layout(&self.commits);
        self.graph_state.selected = 0;
        self.graph_state.offset = 0;
        self.selected_message = None;
        self.selected_diff = None;
        Ok(())
    }

    /// Reload data while preserving the current selection by OID.
    pub fn refresh(&mut self) {
        let selected_oid = self.selected_commit().map(|c| c.oid);

        if self.reload().is_ok() {
            // Restore selection to the same commit if it still exists
            if let Some(oid) = selected_oid {
                if let Some(pos) = self.commits.iter().position(|c| c.oid == oid) {
                    self.graph_state.selected = pos;
                }
            }
            // Clamp selection
            if !self.commits.is_empty() {
                self.graph_state.selected =
                    self.graph_state.selected.min(self.commits.len() - 1);
            }

            // Update branch name
            if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
                self.branch_name = repo
                    .head()
                    .ok()
                    .and_then(|h| h.shorthand().map(String::from))
                    .unwrap_or_else(|| "HEAD (detached)".to_string());
            }
        }
    }
}
