use anyhow::Result;
use git2::{Repository, Status, StatusOptions};

#[derive(Debug, Clone)]
pub struct FileStatus {
    pub path: String,
    pub status_code: FileStatusCode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileStatusCode {
    Modified,
    Added,
    Deleted,
    Renamed,
    Typechange,
    Untracked,
}

impl FileStatusCode {
    pub fn letter(&self) -> &'static str {
        match self {
            FileStatusCode::Modified => "M",
            FileStatusCode::Added => "A",
            FileStatusCode::Deleted => "D",
            FileStatusCode::Renamed => "R",
            FileStatusCode::Typechange => "T",
            FileStatusCode::Untracked => "?",
        }
    }
}

/// Return (unstaged_files, staged_files).
pub fn get_status(repo: &Repository) -> Result<(Vec<FileStatus>, Vec<FileStatus>)> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);

    let statuses = repo.statuses(Some(&mut opts))?;

    let mut unstaged = Vec::new();
    let mut staged = Vec::new();

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let st = entry.status();

        // Index (staged) changes
        if st.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        ) {
            let code = if st.contains(Status::INDEX_NEW) {
                FileStatusCode::Added
            } else if st.contains(Status::INDEX_MODIFIED) {
                FileStatusCode::Modified
            } else if st.contains(Status::INDEX_DELETED) {
                FileStatusCode::Deleted
            } else if st.contains(Status::INDEX_RENAMED) {
                FileStatusCode::Renamed
            } else {
                FileStatusCode::Typechange
            };
            staged.push(FileStatus {
                path: path.clone(),
                status_code: code,
            });
        }

        // Workdir (unstaged) changes
        if st.intersects(
            Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_TYPECHANGE
                | Status::WT_RENAMED,
        ) {
            let code = if st.contains(Status::WT_MODIFIED) {
                FileStatusCode::Modified
            } else if st.contains(Status::WT_DELETED) {
                FileStatusCode::Deleted
            } else if st.contains(Status::WT_RENAMED) {
                FileStatusCode::Renamed
            } else {
                FileStatusCode::Typechange
            };
            unstaged.push(FileStatus {
                path: path.clone(),
                status_code: code,
            });
        }

        // Untracked
        if st.contains(Status::WT_NEW) {
            unstaged.push(FileStatus {
                path,
                status_code: FileStatusCode::Untracked,
            });
        }
    }

    Ok((unstaged, staged))
}

/// Stage a single file (add to index).
pub fn stage_file(repo: &Repository, path: &str) -> Result<()> {
    let mut index = repo.index()?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("bare repo"))?;
    if workdir.join(path).exists() {
        index.add_path(std::path::Path::new(path))?;
    } else {
        index.remove_path(std::path::Path::new(path))?;
    }
    index.write()?;
    Ok(())
}

/// Unstage a single file (reset index entry to HEAD).
pub fn unstage_file(repo: &Repository, path: &str) -> Result<()> {
    if let Ok(head) = repo.head() {
        let tree = head.peel_to_tree()?;
        repo.reset_default(Some(&tree.into_object()), [path])?;
    } else {
        // Initial commit: remove from index entirely
        let mut index = repo.index()?;
        index.remove_path(std::path::Path::new(path))?;
        index.write()?;
    }
    Ok(())
}

/// Stage all files.
pub fn stage_all(repo: &Repository) -> Result<()> {
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;
    Ok(())
}

/// Unstage all files (reset index to HEAD).
pub fn unstage_all(repo: &Repository) -> Result<()> {
    if let Ok(head) = repo.head() {
        let commit = head.peel_to_commit()?;
        let tree = commit.tree()?;
        repo.reset_default(Some(&tree.into_object()), ["*"])?;
    } else {
        // Initial commit: clear the index
        let mut index = repo.index()?;
        index.clear()?;
        index.write()?;
    }
    Ok(())
}

/// Restore a file to its HEAD state (discard working tree changes).
/// For untracked files, this deletes the file.
pub fn restore_file(repo: &Repository, path: &str) -> Result<()> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("bare repo"))?;
    let full_path = workdir.join(path);

    if let Ok(head) = repo.head() {
        let tree = head.peel_to_tree()?;
        if tree.get_path(std::path::Path::new(path)).is_ok() {
            // File exists in HEAD — checkout from HEAD
            repo.checkout_tree(
                &tree.into_object(),
                Some(
                    git2::build::CheckoutBuilder::new()
                        .force()
                        .path(path),
                ),
            )?;
        } else {
            // File doesn't exist in HEAD (untracked) — delete it
            if full_path.exists() {
                std::fs::remove_file(&full_path)?;
            }
        }
    } else {
        // No HEAD (initial commit) — just delete the file
        if full_path.exists() {
            std::fs::remove_file(&full_path)?;
        }
    }
    Ok(())
}

/// Create a commit with the current index and given message.
pub fn create_commit(repo: &Repository, message: &str) -> Result<()> {
    let sig = repo.signature()?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    if let Ok(head) = repo.head() {
        let parent = head.peel_to_commit()?;
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;
    } else {
        // Initial commit -- no parents
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?;
    }
    Ok(())
}
