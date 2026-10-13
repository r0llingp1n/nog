use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use git2::Repository;

/// Fetch every configured remote with pruning, delegating to the system `git`.
///
/// Network operations are handed to the system `git` rather than libgit2's
/// in-process transport so the user's full configuration is honored: `~/.ssh/config`
/// host aliases and identity files, credential helpers, and `[url]` rewrites.
/// libgit2's transport cannot resolve `~/.ssh/config` host aliases (for example
/// `Host github-r0llingp1n` -> `github.com`), which is exactly what a remote
/// like `github-r0llingp1n:r0llingp1n/nog.git` needs.
pub fn fetch_all_prune(repo: &Repository) -> Result<()> {
    // Run in the working tree when present so git loads the repo's config; for a
    // bare repo, use the git directory itself.
    let dir: PathBuf = repo
        .workdir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| repo.path().to_path_buf());

    let output = Command::new("git")
        .current_dir(&dir)
        .args(["fetch", "--all", "--prune"])
        .output()
        .with_context(|| "failed to spawn `git fetch`")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_default();
        anyhow::bail!(
            "git fetch failed: {}",
            if err.is_empty() {
                "unknown error".to_string()
            } else {
                err
            }
        );
    }
    Ok(())
}
