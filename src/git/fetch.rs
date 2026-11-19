use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use git2::Repository;

/// Fetch every configured remote, delegating to the system `git`.
///
/// When `prune` is true, stale remote-tracking branches are removed
/// (`git fetch --all --prune`); otherwise they are kept (`git fetch --all`).
///
/// Network operations are handed to the system `git` rather than libgit2's
/// in-process transport so the user's full configuration is honored: `~/.ssh/config`
/// host aliases and identity files, credential helpers, and `[url]` rewrites.
/// libgit2's transport cannot resolve `~/.ssh/config` host aliases (for example
/// `Host github-r0llingp1n` -> `github.com`), which is exactly what a remote
/// like `github-r0llingp1n:r0llingp1n/nog.git` needs.
pub fn fetch_all(repo: &Repository, prune: bool) -> Result<()> {
    // Run in the working tree when present so git loads the repo's config; for a
    // bare repo, use the git directory itself.
    let dir: PathBuf = repo
        .workdir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| repo.path().to_path_buf());

    let mut cmd = Command::new("git");
    cmd.current_dir(&dir).args(["fetch", "--all"]);
    if prune {
        cmd.arg("--prune");
    }
    let output = cmd
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

#[cfg(test)]
mod tests {
    use super::fetch_all;
    use std::path::Path;
    use std::process::Command;

    fn git(args: &[&str], cwd: &Path) {
        let status = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .expect("git not found or failed");
        assert!(status.success(), "git {:?} failed in {:?}", args, cwd);
    }

    /// A local repo with a bare `origin` at a shared base commit.
    fn local_with_remote(tag: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("nog_fetch_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let local = base.join("local");
        let remote = base.join("remote.git");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::create_dir_all(&remote).unwrap();

        git(&["init"], &local);
        git(&["init", "--bare"], &remote);
        git(
            &["remote", "add", "origin", remote.to_str().unwrap()],
            &local,
        );
        git(&["config", "user.email", "t@example.com"], &local);
        git(&["config", "user.name", "Tester"], &local);
        // Keep a plain `git fetch` from pruning so the no-prune test is
        // deterministic regardless of the machine's `fetch.prune` default.
        git(&["config", "remote.origin.prune", "false"], &local);

        std::fs::write(local.join("file.txt"), "base").unwrap();
        git(&["add", "file.txt"], &local);
        git(&["commit", "-m", "base"], &local);
        git(&["push", "-u", "origin", "HEAD"], &local);
        local
    }

    /// Like [`local_with_remote`], but a `feature` branch has been created on
    /// the remote, fetched into local (so `refs/remotes/origin/feature` exists),
    /// and then deleted from the remote — leaving a stale local tracking ref that
    /// only `--prune` should remove.
    fn local_with_stale_remote_branch(tag: &str) -> std::path::PathBuf {
        let local = local_with_remote(tag);
        let base = local.parent().unwrap();
        let remote = base.join("remote.git");
        let fork = base.join("fork");
        let _ = std::fs::remove_dir_all(&fork);
        git(
            &["clone", remote.to_str().unwrap(), fork.to_str().unwrap()],
            base,
        );
        git(&["config", "user.email", "t@example.com"], &fork);
        git(&["config", "user.name", "Tester"], &fork);
        git(&["branch", "feature", "HEAD"], &fork);
        git(&["push", "origin", "feature"], &fork);

        // Fetch so local tracks origin/feature.
        {
            let repo = git2::Repository::open(&local).unwrap();
            fetch_all(&repo, false).expect("setup fetch failed");
        }
        let repo = git2::Repository::open(&local).unwrap();
        assert!(
            repo.find_reference("refs/remotes/origin/feature").is_ok(),
            "origin/feature should be tracked after the setup fetch"
        );

        // Delete the branch on the remote, leaving the local tracking ref stale.
        git(&["push", "origin", "--delete", "feature"], &fork);
        local
    }

    #[test]
    fn prune_removes_stale_remote_tracking_ref() {
        let local = local_with_stale_remote_branch("prune");
        let repo = git2::Repository::open(&local).unwrap();
        fetch_all(&repo, true).expect("prune fetch failed");
        let repo = git2::Repository::open(&local).unwrap();
        assert!(
            repo.find_reference("refs/remotes/origin/feature").is_err(),
            "stale origin/feature should be removed by fetch --all --prune"
        );
    }

    #[test]
    fn no_prune_keeps_stale_remote_tracking_ref() {
        let local = local_with_stale_remote_branch("noprun");
        let repo = git2::Repository::open(&local).unwrap();
        fetch_all(&repo, false).expect("fetch failed");
        let repo = git2::Repository::open(&local).unwrap();
        assert!(
            repo.find_reference("refs/remotes/origin/feature").is_ok(),
            "without --prune the stale origin/feature should remain"
        );
    }
}
