use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use git2::{BranchType, Oid, Repository};

use super::types::{BranchInfo, Decorations, TagInfo, WorktreeInfo};

pub fn open_repo(path: &Path) -> Result<Repository> {
    Ok(Repository::discover(path)?)
}

pub fn load_decorations(repo: &Repository) -> Result<HashMap<Oid, Decorations>> {
    let mut map: HashMap<Oid, Decorations> = HashMap::new();
    let head_oid = repo.head().ok().and_then(|h| h.target());
    let head_name = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(String::from));

    // Branches
    for branch_result in repo.branches(None)? {
        let (branch, branch_type) = branch_result?;
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => continue,
        };
        let target = match branch.get().peel_to_commit() {
            Ok(c) => c.id(),
            Err(_) => continue,
        };
        let is_head = head_oid == Some(target)
            && branch_type == BranchType::Local
            && head_name.as_deref() == Some(&name);

        let entry = map.entry(target).or_default();
        entry.branches.push(BranchInfo {
            name,
            is_remote: branch_type == BranchType::Remote,
            is_head,
        });
        if is_head {
            entry.is_head = true;
        }
    }

    // Tags
    repo.tag_foreach(|oid, name_bytes| {
        let name = String::from_utf8_lossy(name_bytes)
            .trim_start_matches("refs/tags/")
            .to_string();
        if let Ok(obj) = repo.find_object(oid, None) {
            if let Ok(commit) = obj.peel_to_commit() {
                map.entry(commit.id())
                    .or_default()
                    .tags
                    .push(TagInfo { name });
            }
        }
        true
    })?;

    // Worktrees: each worktree is a live checkout pinned to some commit.
    // Surface its location as a decoration on that commit, the same way
    // branches and tags are attached, so 'w' can reveal them in the DAG.
    //
    // git2 0.19's worktrees() omits the main worktree, so we include it
    // explicitly from the repo's own working directory and HEAD.
    let main_root = repo
        .workdir()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut labels: Vec<(Oid, String)> = Vec::new();

    if let Some(oid) = repo.head().ok().and_then(|h| h.target()) {
        if let Some(wt_path) = repo.workdir() {
            labels.push((oid, worktree_label(&main_root, wt_path)));
        }
    }

    if let Ok(worktrees) = repo.worktrees() {
        for name in worktrees.iter().flatten() {
            let worktree = match repo.find_worktree(name) {
                Ok(worktree) => worktree,
                Err(_) => continue,
            };
            // The commit the worktree is checked out on, via the worktree's HEAD.
            let wt_repo = match git2::Repository::open_from_worktree(&worktree) {
                Ok(wt_repo) => wt_repo,
                Err(_) => continue,
            };
            let Some(oid) = wt_repo.head().ok().and_then(|h| h.target()) else {
                continue;
            };
            labels.push((oid, worktree_label(&main_root, worktree.path())));
        }
    }

    // Attach, deduplicating a label per commit (the main worktree can also
    // appear in the worktrees() list on some git versions).
    for (oid, label) in labels {
        let entry = map.entry(oid).or_default();
        if !entry.worktrees.iter().any(|w| w.name == label) {
            entry.worktrees.push(WorktreeInfo { name: label });
        }
    }

    Ok(map)
}

/// A compact label for a worktree: its path relative to the main worktree's
/// parent directory (e.g. `nog-sprint-9`), falling back to the directory
/// name or the full path when no common parent can be derived.
fn worktree_label(main_root: &Option<PathBuf>, wt_path: &Path) -> String {
    if let Some(root) = main_root {
        if let Ok(rel) = wt_path.strip_prefix(root) {
            let rel = rel.to_string_lossy();
            let rel = rel.trim_end_matches('/');
            if !rel.is_empty() {
                return rel.to_string();
            }
        }
    }
    wt_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| wt_path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_uses_relative_path_under_common_parent() {
        assert_eq!(
            worktree_label(&Some("/a/b".into()), Path::new("/a/b/nog-sprint-9")),
            "nog-sprint-9"
        );
    }

    #[test]
    fn label_strips_trailing_separator() {
        assert_eq!(
            worktree_label(&Some("/a".into()), Path::new("/a/sub/")),
            "sub"
        );
    }

    #[test]
    fn label_falls_back_to_dir_name_outside_root() {
        assert_eq!(
            worktree_label(&Some("/a/b".into()), Path::new("/x/y/foo")),
            "foo"
        );
    }

    #[test]
    fn label_falls_back_to_dir_name_without_root() {
        assert_eq!(worktree_label(&None, Path::new("/x/y/foo")), "foo");
    }

    #[test]
    fn label_uses_full_path_for_rootless_path() {
        // A path with no directory component yields the path itself.
        assert_eq!(worktree_label(&None, Path::new("solo")), "solo");
    }
}
