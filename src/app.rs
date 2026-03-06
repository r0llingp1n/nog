use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use git2::Oid;

use crate::git::diff::{self, DiffData};
use crate::git::log;
use crate::git::repo;

pub struct BranchInfo {
    pub name: String,
    pub commit_summary: String,
    pub author: String,
}
use crate::git::stash::{self, StashEntry};
use crate::git::status::{self, FileStatus};
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
    CommitView,
    StashView,
}

#[derive(PartialEq, Clone, Copy)]
pub enum CommitFocus {
    UnstagedFiles,
    StagedFiles,
    MessageEditor,
    DiffPreview,
}

#[derive(PartialEq, Clone, Copy)]
pub enum StashViewFocus {
    StashList,
    FileList,
    DiffPreview,
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
    pub branches: Vec<BranchInfo>,
    pub branch_selected: usize,
    pub branch_error: Option<String>,

    // Stash prompt state
    pub stash_input: String,
    pub stash_target_branch: Option<String>,
    pub stash_error: Option<String>,

    // Commit view state
    pub commit_focus: CommitFocus,
    pub commit_unstaged: Vec<FileStatus>,
    pub commit_staged: Vec<FileStatus>,
    pub commit_unstaged_selected: usize,
    pub commit_staged_selected: usize,
    pub commit_message: String,
    pub commit_error: Option<String>,
    pub commit_diff: Option<DiffData>,
    pub commit_diff_scroll: u16,
    pub commit_diff_path: String,
    pub commit_diff_from: CommitFocus,

    // Stash view state
    pub stash_entries: Vec<StashEntry>,
    pub stash_view_focus: StashViewFocus,
    pub stash_selected: usize,
    pub stash_files: Vec<FileStatus>,
    pub stash_file_selected: usize,
    pub stash_diff: Option<DiffData>,
    pub stash_diff_scroll: u16,
    pub stash_diff_path: String,
    pub stash_view_error: Option<String>,

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
            commit_focus: CommitFocus::UnstagedFiles,
            commit_unstaged: Vec::new(),
            commit_staged: Vec::new(),
            commit_unstaged_selected: 0,
            commit_staged_selected: 0,
            commit_message: String::new(),
            commit_error: None,
            commit_diff: None,
            commit_diff_scroll: 0,
            commit_diff_path: String::new(),
            commit_diff_from: CommitFocus::UnstagedFiles,
            stash_entries: Vec::new(),
            stash_view_focus: StashViewFocus::StashList,
            stash_selected: 0,
            stash_files: Vec::new(),
            stash_file_selected: 0,
            stash_diff: None,
            stash_diff_scroll: 0,
            stash_diff_path: String::new(),
            stash_view_error: None,
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
            AppMode::CommitView => self.handle_commit_view_key(key),
            AppMode::StashView => self.handle_stash_view_key(key),
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
            KeyCode::Char('c') => {
                self.open_commit_view();
            }
            KeyCode::Char('s') => {
                self.open_stash_view();
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
            let mut branches: Vec<BranchInfo> = Vec::new();
            if let Ok(iter) = repo.branches(Some(git2::BranchType::Local)) {
                for branch_result in iter {
                    if let Ok((branch, _)) = branch_result {
                        if let Ok(Some(name)) = branch.name() {
                            let (commit_summary, author) = branch
                                .get()
                                .peel_to_commit()
                                .ok()
                                .map(|c| {
                                    let summary = c.summary().unwrap_or("").to_string();
                                    let author = c.author().name().unwrap_or("").to_string();
                                    (summary, author)
                                })
                                .unwrap_or_default();
                            branches.push(BranchInfo {
                                name: name.to_string(),
                                commit_summary,
                                author,
                            });
                        }
                    }
                }
            }
            branches.sort_by(|a, b| a.name.cmp(&b.name));
            // Pre-select current branch
            let selected = branches
                .iter()
                .position(|b| b.name == self.branch_name)
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
            Some(info) => info.name.clone(),
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

    fn open_commit_view(&mut self) {
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            match status::get_status(&repo) {
                Ok((unstaged, staged)) => {
                    self.commit_unstaged = unstaged;
                    self.commit_staged = staged;
                    self.commit_unstaged_selected = 0;
                    self.commit_staged_selected = 0;
                    self.commit_message.clear();
                    self.commit_error = None;
                    self.commit_focus = CommitFocus::UnstagedFiles;
                    self.mode = AppMode::CommitView;
                }
                Err(e) => {
                    self.commit_error = Some(format!("Failed to load status: {}", e));
                }
            }
        }
    }

    fn handle_commit_view_key(&mut self, key: KeyEvent) {
        // Diff preview handles its own keys exclusively
        if self.commit_focus == CommitFocus::DiffPreview {
            self.handle_commit_diff_key(key);
            return;
        }

        // Universal keys
        match key.code {
            KeyCode::Tab => {
                self.commit_focus = match self.commit_focus {
                    CommitFocus::UnstagedFiles => CommitFocus::StagedFiles,
                    CommitFocus::StagedFiles => CommitFocus::MessageEditor,
                    CommitFocus::MessageEditor => CommitFocus::UnstagedFiles,
                    CommitFocus::DiffPreview => unreachable!(),
                };
                return;
            }
            KeyCode::BackTab => {
                self.commit_focus = match self.commit_focus {
                    CommitFocus::UnstagedFiles => CommitFocus::MessageEditor,
                    CommitFocus::StagedFiles => CommitFocus::UnstagedFiles,
                    CommitFocus::MessageEditor => CommitFocus::StagedFiles,
                    CommitFocus::DiffPreview => unreachable!(),
                };
                return;
            }
            _ => {}
        }

        match self.commit_focus {
            CommitFocus::UnstagedFiles => self.handle_commit_unstaged_key(key),
            CommitFocus::StagedFiles => self.handle_commit_staged_key(key),
            CommitFocus::MessageEditor => self.handle_commit_message_key(key),
            CommitFocus::DiffPreview => unreachable!(),
        }
    }

    fn handle_commit_unstaged_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.mode = AppMode::GraphView;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.commit_unstaged.is_empty()
                    && self.commit_unstaged_selected + 1 < self.commit_unstaged.len()
                {
                    self.commit_unstaged_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.commit_unstaged_selected = self.commit_unstaged_selected.saturating_sub(1);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.commit_unstaged_selected = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                if !self.commit_unstaged.is_empty() {
                    self.commit_unstaged_selected = self.commit_unstaged.len() - 1;
                }
            }
            KeyCode::Char('s') => {
                self.stage_selected_file();
            }
            KeyCode::Char(' ') => {
                self.open_commit_diff(false);
            }
            KeyCode::Char('r') => {
                self.restore_selected_file();
            }
            KeyCode::Char('a') => {
                self.stage_all_files();
            }
            KeyCode::Char('c') => {
                self.perform_commit();
            }
            _ => {}
        }
    }

    fn handle_commit_staged_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.mode = AppMode::GraphView;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.commit_staged.is_empty()
                    && self.commit_staged_selected + 1 < self.commit_staged.len()
                {
                    self.commit_staged_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.commit_staged_selected = self.commit_staged_selected.saturating_sub(1);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.commit_staged_selected = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                if !self.commit_staged.is_empty() {
                    self.commit_staged_selected = self.commit_staged.len() - 1;
                }
            }
            KeyCode::Char('u') => {
                self.unstage_selected_file();
            }
            KeyCode::Char(' ') => {
                self.open_commit_diff(true);
            }
            KeyCode::Char('a') => {
                self.unstage_all_files();
            }
            KeyCode::Char('c') => {
                self.perform_commit();
            }
            _ => {}
        }
    }

    fn handle_commit_message_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.commit_focus = CommitFocus::StagedFiles;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.perform_commit();
            }
            KeyCode::Enter => {
                self.commit_message.push('\n');
            }
            KeyCode::Backspace => {
                self.commit_message.pop();
            }
            KeyCode::Char(c) => {
                self.commit_message.push(c);
            }
            _ => {}
        }
    }

    fn open_commit_diff(&mut self, staged: bool) {
        let (files, selected) = if staged {
            (&self.commit_staged, self.commit_staged_selected)
        } else {
            (&self.commit_unstaged, self.commit_unstaged_selected)
        };
        let path = match files.get(selected) {
            Some(f) => f.path.clone(),
            None => return,
        };
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            match diff::diff_workdir_file(&repo, &path, staged) {
                Ok(data) => {
                    self.commit_diff_from = self.commit_focus;
                    self.commit_diff = Some(data);
                    self.commit_diff_scroll = 0;
                    self.commit_diff_path = path;
                    self.commit_focus = CommitFocus::DiffPreview;
                }
                Err(e) => {
                    self.commit_error = Some(format!("Diff failed: {}", e));
                }
            }
        }
    }

    fn handle_commit_diff_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char(' ') => {
                self.commit_focus = self.commit_diff_from;
                self.commit_diff = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.commit_diff_scroll = self.commit_diff_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.commit_diff_scroll = self.commit_diff_scroll.saturating_sub(1);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.commit_diff_scroll = self.commit_diff_scroll.saturating_add(20);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.commit_diff_scroll = self.commit_diff_scroll.saturating_sub(20);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.commit_diff_scroll = 0;
            }
            _ => {}
        }
    }

    fn stage_selected_file(&mut self) {
        let path = match self.commit_unstaged.get(self.commit_unstaged_selected) {
            Some(f) => f.path.clone(),
            None => return,
        };
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = status::stage_file(&repo, &path) {
                self.commit_error = Some(format!("Stage failed: {}", e));
                return;
            }
        }
        self.refresh_commit_status();
        // If unstaged is now empty, move focus to staged
        if self.commit_unstaged.is_empty() {
            self.commit_focus = CommitFocus::StagedFiles;
        }
    }

    fn unstage_selected_file(&mut self) {
        let path = match self.commit_staged.get(self.commit_staged_selected) {
            Some(f) => f.path.clone(),
            None => return,
        };
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = status::unstage_file(&repo, &path) {
                self.commit_error = Some(format!("Unstage failed: {}", e));
                return;
            }
        }
        self.refresh_commit_status();
        // If staged is now empty, move focus to unstaged
        if self.commit_staged.is_empty() {
            self.commit_focus = CommitFocus::UnstagedFiles;
        }
    }

    fn restore_selected_file(&mut self) {
        let path = match self.commit_unstaged.get(self.commit_unstaged_selected) {
            Some(f) => f.path.clone(),
            None => return,
        };
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = status::restore_file(&repo, &path) {
                self.commit_error = Some(format!("Restore failed: {}", e));
                return;
            }
        }
        self.refresh_commit_status();
        if self.commit_unstaged_selected > 0
            && self.commit_unstaged_selected >= self.commit_unstaged.len()
        {
            self.commit_unstaged_selected = self.commit_unstaged.len().saturating_sub(1);
        }
    }

    fn stage_all_files(&mut self) {
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = status::stage_all(&repo) {
                self.commit_error = Some(format!("Stage all failed: {}", e));
                return;
            }
        }
        self.refresh_commit_status();
        self.commit_focus = CommitFocus::StagedFiles;
    }

    fn unstage_all_files(&mut self) {
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = status::unstage_all(&repo) {
                self.commit_error = Some(format!("Unstage all failed: {}", e));
                return;
            }
        }
        self.refresh_commit_status();
        self.commit_focus = CommitFocus::UnstagedFiles;
    }

    fn perform_commit(&mut self) {
        if self.commit_staged.is_empty() {
            self.commit_error = Some("Nothing staged to commit".to_string());
            return;
        }
        let msg = self.commit_message.trim().to_string();
        if msg.is_empty() {
            self.commit_error = Some("Commit message cannot be empty".to_string());
            return;
        }
        let result = (|| -> Result<()> {
            let repo = git2::Repository::open(&self.repo_internal_path)?;
            status::create_commit(&repo, &msg)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.commit_message.clear();
                self.commit_error = None;
                let _ = self.reload();
                self.mode = AppMode::GraphView;
            }
            Err(e) => {
                self.commit_error = Some(format!("Commit failed: {}", e));
            }
        }
    }

    fn refresh_commit_status(&mut self) {
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Ok((unstaged, staged)) = status::get_status(&repo) {
                self.commit_unstaged = unstaged;
                self.commit_staged = staged;
                if !self.commit_unstaged.is_empty() {
                    self.commit_unstaged_selected = self
                        .commit_unstaged_selected
                        .min(self.commit_unstaged.len() - 1);
                } else {
                    self.commit_unstaged_selected = 0;
                }
                if !self.commit_staged.is_empty() {
                    self.commit_staged_selected = self
                        .commit_staged_selected
                        .min(self.commit_staged.len() - 1);
                } else {
                    self.commit_staged_selected = 0;
                }
            }
        }
        self.commit_error = None;
    }

    fn open_stash_view(&mut self) {
        if let Ok(mut repo) = git2::Repository::open(&self.repo_internal_path) {
            match stash::list_stashes(&mut repo) {
                Ok(entries) => {
                    self.stash_entries = entries;
                    self.stash_selected = 0;
                    self.stash_view_focus = StashViewFocus::StashList;
                    self.stash_view_error = None;
                    self.stash_diff = None;
                    self.load_stash_files();
                    self.mode = AppMode::StashView;
                }
                Err(e) => {
                    self.stash_view_error = Some(format!("Failed to list stashes: {}", e));
                }
            }
        }
    }

    fn load_stash_files(&mut self) {
        if let Some(entry) = self.stash_entries.get(self.stash_selected) {
            let oid = entry.oid;
            if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
                match stash::stash_files(&repo, oid) {
                    Ok(files) => {
                        self.stash_files = files;
                        self.stash_file_selected = 0;
                    }
                    Err(_) => {
                        self.stash_files = Vec::new();
                        self.stash_file_selected = 0;
                    }
                }
            }
        } else {
            self.stash_files = Vec::new();
            self.stash_file_selected = 0;
        }
    }

    fn handle_stash_view_key(&mut self, key: KeyEvent) {
        if self.stash_view_focus == StashViewFocus::DiffPreview {
            self.handle_stash_diff_key(key);
            return;
        }

        match self.stash_view_focus {
            StashViewFocus::StashList => self.handle_stash_list_key(key),
            StashViewFocus::FileList => self.handle_stash_file_list_key(key),
            StashViewFocus::DiffPreview => unreachable!(),
        }
    }

    fn handle_stash_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.mode = AppMode::GraphView;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.stash_entries.is_empty()
                    && self.stash_selected + 1 < self.stash_entries.len()
                {
                    self.stash_selected += 1;
                    self.load_stash_files();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.stash_selected > 0 {
                    self.stash_selected -= 1;
                    self.load_stash_files();
                }
            }
            KeyCode::Char('g') | KeyCode::Home => {
                if self.stash_selected != 0 {
                    self.stash_selected = 0;
                    self.load_stash_files();
                }
            }
            KeyCode::Char('G') | KeyCode::End => {
                if !self.stash_entries.is_empty() {
                    let last = self.stash_entries.len() - 1;
                    if self.stash_selected != last {
                        self.stash_selected = last;
                        self.load_stash_files();
                    }
                }
            }
            KeyCode::Char(' ') => {
                if !self.stash_files.is_empty() {
                    self.stash_view_focus = StashViewFocus::FileList;
                }
            }
            KeyCode::Char('a') => {
                self.apply_selected_stash();
            }
            KeyCode::Char('p') => {
                self.pop_selected_stash();
            }
            KeyCode::Char('d') => {
                self.drop_selected_stash();
            }
            _ => {}
        }
    }

    fn handle_stash_file_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.stash_view_focus = StashViewFocus::StashList;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.stash_files.is_empty()
                    && self.stash_file_selected + 1 < self.stash_files.len()
                {
                    self.stash_file_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.stash_file_selected = self.stash_file_selected.saturating_sub(1);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.stash_file_selected = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                if !self.stash_files.is_empty() {
                    self.stash_file_selected = self.stash_files.len() - 1;
                }
            }
            KeyCode::Char(' ') => {
                self.open_stash_file_diff();
            }
            _ => {}
        }
    }

    fn handle_stash_diff_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char(' ') => {
                self.stash_view_focus = StashViewFocus::FileList;
                self.stash_diff = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.stash_diff_scroll = self.stash_diff_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.stash_diff_scroll = self.stash_diff_scroll.saturating_sub(1);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.stash_diff_scroll = self.stash_diff_scroll.saturating_add(20);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.stash_diff_scroll = self.stash_diff_scroll.saturating_sub(20);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.stash_diff_scroll = 0;
            }
            _ => {}
        }
    }

    fn open_stash_file_diff(&mut self) {
        let oid = match self.stash_entries.get(self.stash_selected) {
            Some(e) => e.oid,
            None => return,
        };
        let path = match self.stash_files.get(self.stash_file_selected) {
            Some(f) => f.path.clone(),
            None => return,
        };
        if let Ok(repo) = git2::Repository::open(&self.repo_internal_path) {
            match stash::stash_file_diff(&repo, oid, &path) {
                Ok(data) => {
                    self.stash_diff = Some(data);
                    self.stash_diff_scroll = 0;
                    self.stash_diff_path = path;
                    self.stash_view_focus = StashViewFocus::DiffPreview;
                }
                Err(e) => {
                    self.stash_view_error = Some(format!("Diff failed: {}", e));
                }
            }
        }
    }

    fn apply_selected_stash(&mut self) {
        let index = match self.stash_entries.get(self.stash_selected) {
            Some(e) => e.index,
            None => return,
        };
        if let Ok(mut repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = stash::apply_stash(&mut repo, index) {
                self.stash_view_error = Some(format!("Apply failed: {}", e));
                return;
            }
        }
        self.stash_view_error = None;
        self.refresh_stash_list();
    }

    fn pop_selected_stash(&mut self) {
        let index = match self.stash_entries.get(self.stash_selected) {
            Some(e) => e.index,
            None => return,
        };
        if let Ok(mut repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = stash::pop_stash(&mut repo, index) {
                self.stash_view_error = Some(format!("Pop failed: {}", e));
                return;
            }
        }
        self.stash_view_error = None;
        self.refresh_stash_list();
    }

    fn drop_selected_stash(&mut self) {
        let index = match self.stash_entries.get(self.stash_selected) {
            Some(e) => e.index,
            None => return,
        };
        if let Ok(mut repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Err(e) = stash::drop_stash(&mut repo, index) {
                self.stash_view_error = Some(format!("Drop failed: {}", e));
                return;
            }
        }
        self.stash_view_error = None;
        self.refresh_stash_list();
    }

    fn refresh_stash_list(&mut self) {
        if let Ok(mut repo) = git2::Repository::open(&self.repo_internal_path) {
            if let Ok(entries) = stash::list_stashes(&mut repo) {
                self.stash_entries = entries;
                if !self.stash_entries.is_empty() {
                    self.stash_selected = self
                        .stash_selected
                        .min(self.stash_entries.len() - 1);
                } else {
                    self.stash_selected = 0;
                }
                self.load_stash_files();
            }
        }
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
