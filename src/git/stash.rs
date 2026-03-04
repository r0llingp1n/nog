use anyhow::Result;
use git2::{DiffOptions, Oid, Repository};

use super::diff::{DiffData, DiffFile, DiffHunk, DiffLine};
use super::status::{FileStatus, FileStatusCode};

#[derive(Debug, Clone)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub oid: Oid,
}

/// List all stash entries.
pub fn list_stashes(repo: &mut Repository) -> Result<Vec<StashEntry>> {
    let mut entries = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        entries.push(StashEntry {
            index,
            message: message.to_string(),
            oid: *oid,
        });
        true
    })?;
    Ok(entries)
}

/// Get the list of files changed in a stash entry.
pub fn stash_files(repo: &Repository, oid: Oid) -> Result<Vec<FileStatus>> {
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = DiffOptions::new();
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;

    let mut files = Vec::new();
    for delta in diff.deltas() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let status_code = match delta.status() {
            git2::Delta::Added => FileStatusCode::Added,
            git2::Delta::Deleted => FileStatusCode::Deleted,
            git2::Delta::Modified => FileStatusCode::Modified,
            git2::Delta::Renamed => FileStatusCode::Renamed,
            git2::Delta::Typechange => FileStatusCode::Typechange,
            _ => FileStatusCode::Modified,
        };

        files.push(FileStatus { path, status_code });
    }

    Ok(files)
}

/// Get the diff for a single file within a stash entry.
pub fn stash_file_diff(repo: &Repository, oid: Oid, path: &str) -> Result<DiffData> {
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    opts.pathspec(path);

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;

    let mut files: Vec<DiffFile> = Vec::new();
    for delta_idx in 0..diff.deltas().len() {
        let patch = git2::Patch::from_diff(&diff, delta_idx)?;
        if let Some(patch) = patch {
            let delta = diff.get_delta(delta_idx).unwrap();
            let file_path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let mut hunks = Vec::new();
            for hunk_idx in 0..patch.num_hunks() {
                let (hunk, _) = patch.hunk(hunk_idx)?;
                let header = String::from_utf8_lossy(hunk.header()).trim().to_string();
                let mut lines = Vec::new();

                for line_idx in 0..patch.num_lines_in_hunk(hunk_idx)? {
                    let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                    lines.push(DiffLine {
                        origin: line.origin(),
                        content: String::from_utf8_lossy(line.content()).to_string(),
                    });
                }

                hunks.push(DiffHunk { header, lines });
            }

            files.push(DiffFile {
                path: file_path,
                hunks,
            });
        }
    }

    Ok(DiffData { files })
}

pub fn apply_stash(repo: &mut Repository, index: usize) -> Result<()> {
    repo.stash_apply(index, None)?;
    Ok(())
}

pub fn pop_stash(repo: &mut Repository, index: usize) -> Result<()> {
    repo.stash_pop(index, None)?;
    Ok(())
}

pub fn drop_stash(repo: &mut Repository, index: usize) -> Result<()> {
    repo.stash_drop(index)?;
    Ok(())
}
