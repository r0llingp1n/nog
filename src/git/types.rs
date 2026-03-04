use chrono::{DateTime, Local};
use git2::Oid;

/// Lightweight commit info extracted from git2::Commit.
/// We copy out the data so we don't hold borrows on the repo.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub oid: Oid,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub author_date: DateTime<Local>,
    pub parent_oids: Vec<Oid>,
}

#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub is_head: bool,
}

#[derive(Debug, Clone)]
pub struct TagInfo {
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct Decorations {
    pub branches: Vec<BranchInfo>,
    pub tags: Vec<TagInfo>,
    pub is_head: bool,
}
