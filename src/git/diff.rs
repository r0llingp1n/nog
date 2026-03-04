use anyhow::Result;
use git2::{DiffOptions, Oid, Repository};

#[derive(Debug, Clone)]
pub struct DiffData {
    pub files: Vec<DiffFile>,
}

#[derive(Debug, Clone)]
pub struct DiffFile {
    pub path: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub origin: char,
    pub content: String,
}

pub fn diff_commit(repo: &Repository, oid: Oid) -> Result<DiffData> {
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = DiffOptions::new();
    opts.context_lines(3);

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;

    let mut files: Vec<DiffFile> = Vec::new();

    // Iterate via patches to avoid multiple mutable borrow issue with foreach
    for delta_idx in 0..diff.deltas().len() {
        let patch = git2::Patch::from_diff(&diff, delta_idx)?;
        if let Some(patch) = patch {
            let delta = diff.get_delta(delta_idx).unwrap();
            let path = delta
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

            files.push(DiffFile { path, hunks });
        }
    }

    Ok(DiffData { files })
}

/// Diff a single file's working-tree or staged (index) changes against HEAD.
pub fn diff_workdir_file(repo: &Repository, path: &str, staged: bool) -> Result<DiffData> {
    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    opts.pathspec(path);

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let diff = if staged {
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?
    };

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

pub fn get_commit_message(repo: &Repository, oid: Oid) -> Result<String> {
    let commit = repo.find_commit(oid)?;
    Ok(commit.message().unwrap_or("").to_string())
}
