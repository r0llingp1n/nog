use std::path::Path;

use anyhow::Result;
use git2::build::CheckoutBuilder;
use git2::{Oid, Repository, ResetType};

use super::fetch;
use super::status;

/// How a sync should reconcile the local branch with its upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullMode {
    /// Lowercase `p`: always produce a merge commit (`git merge --no-ff`
    /// semantics) when the branch has diverged.
    MergeCommit,
    /// Uppercase `P`: fast-forward only; do not create a merge commit.
    FastForwardOnly,
}

/// The terminal result of a sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncResult {
    /// The local branch was already up to date with its upstream.
    UpToDate,
    /// The sync completed and any stashed local changes were re-applied.
    Synced,
    /// Fast-forward-only could not fast-forward because the branch diverged.
    Diverged,
    /// The current branch has no upstream to sync against.
    NoUpstream,
    /// The sync applied but the stashed local changes could not be re-applied
    /// cleanly; the stash was kept for manual resolution.
    StashKept,
    /// The merge left conflicts; the caller should open a conflict view.
    Conflicted,
}

/// The outcome of [`sync`].
#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub result: SyncResult,
    pub message: String,
    /// Pre-merge HEAD; `Some` only for `Conflicted`, so an abort can restore it.
    pub pre_merge_head: Option<Oid>,
    /// Conflicted files; `Some` only for `Conflicted`.
    pub conflicted_files: Vec<String>,
    /// Whether a stash still needs to be popped (true only for `Conflicted`).
    pub stashed: bool,
}

fn current_branch_name(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    let name = head
        .shorthand()
        .ok_or_else(|| anyhow::anyhow!("cannot determine current branch"))?;
    Ok(name.to_string())
}

/// Resolve the upstream ref to sync against: the branch's configured upstream
/// (`HEAD@{upstream}`), falling back to `origin/<branch>`.
fn resolve_upstream(repo: &Repository, branch: &str) -> Result<Option<Oid>> {
    if let Ok(obj) = repo.revparse_single("HEAD@{upstream}") {
        if let Some(commit) = obj.as_commit() {
            return Ok(Some(commit.id()));
        }
    }
    let origin_ref = format!("origin/{}", branch);
    if let Ok(obj) = repo.revparse_single(&origin_ref) {
        if let Some(commit) = obj.as_commit() {
            return Ok(Some(commit.id()));
        }
    }
    Ok(None)
}

/// First 8 hex chars of an OID, for human-readable messages.
fn short(oid: &Oid) -> String {
    let s = format!("{}", oid);
    s[..8].to_string()
}

/// Pop the most recent stash if one was created. Propagates errors.
fn pop_stash_if_needed(repo: &mut Repository, stashed: bool) -> Result<()> {
    if stashed {
        repo.stash_pop(0, None)?;
    }
    Ok(())
}

/// Pop the most recent stash if one was created. Returns `true` if the pop
/// failed, leaving the stash in place for manual resolution.
fn pop_stash_best_effort(repo: &mut Repository, stashed: bool) -> bool {
    if !stashed {
        return false;
    }
    match repo.stash_pop(0, None) {
        Ok(()) => false,
        Err(_) => true,
    }
}

/// Run the full sync sequence for `mode` and return the outcome.
///
/// Local tracked changes are stashed, all remotes are fetched with pruning, and
/// the branch is merged (or fast-forwarded). A conflicted merge is left in
/// progress for a conflict view; every other terminal path re-applies the
/// stash. A failed sync never strands a stash.
pub fn sync(repo_path: &Path, mode: PullMode) -> Result<SyncOutcome> {
    let mut repo = Repository::open(repo_path)?;
    let branch = current_branch_name(&repo)?;
    let pre_merge_head = repo.head()?.target().ok_or_else(|| {
        anyhow::anyhow!("cannot sync in a detached HEAD; check out a branch first")
    })?;

    // 1. Stash local tracked changes so the sync runs on a clean tree.
    let stashed = if status::has_tracked_changes(&repo)? {
        let sig = repo.signature()?;
        repo.stash_save(&sig, "nog: sync", None)?;
        true
    } else {
        false
    };

    // 2. Fetch + reconcile. On any error after stashing, best-effort restore
    // the stash so a failed sync never strands the user's work.
    let result = (|| -> Result<SyncOutcome> {
        fetch::fetch_all_prune(&repo)?;

        let upstream = match resolve_upstream(&repo, &branch)? {
            Some(oid) => oid,
            None => {
                pop_stash_if_needed(&mut repo, stashed)?;
                return Ok(SyncOutcome {
                    result: SyncResult::NoUpstream,
                    message: format!("No upstream configured for '{}'", branch),
                    pre_merge_head: None,
                    conflicted_files: Vec::new(),
                    stashed: false,
                });
            }
        };

        match mode {
            PullMode::FastForwardOnly => sync_fast_forward(&mut repo, &branch, upstream, stashed),
            PullMode::MergeCommit => {
                sync_merge_commit(&mut repo, &branch, pre_merge_head, upstream, stashed)
            }
        }
    })();

    match result {
        Ok(outcome) => Ok(outcome),
        Err(e) => {
            if stashed {
                let _ = repo.stash_pop(0, None);
            }
            Err(e)
        }
    }
}

fn sync_fast_forward(
    repo: &mut Repository,
    branch: &str,
    upstream: Oid,
    stashed: bool,
) -> Result<SyncOutcome> {
    let analysis = {
        let their = repo.find_annotated_commit(upstream)?;
        let (analysis, _preference) = repo.merge_analysis(&[&their])?;
        analysis
    };

    if analysis.is_up_to_date() {
        pop_stash_if_needed(repo, stashed)?;
        return Ok(SyncOutcome {
            result: SyncResult::UpToDate,
            message: format!("'{}' is up to date with its upstream", branch),
            pre_merge_head: None,
            conflicted_files: Vec::new(),
            stashed: false,
        });
    }

    if !analysis.is_fast_forward() {
        pop_stash_if_needed(repo, stashed)?;
        return Ok(SyncOutcome {
            result: SyncResult::Diverged,
            message: format!(
                "'{}' has diverged from its upstream; press 'p' for a merge commit",
                branch
            ),
            pre_merge_head: None,
            conflicted_files: Vec::new(),
            stashed: false,
        });
    }

    // Fast-forward the branch ref, then check out the new tree.
    {
        let ref_name = format!("refs/heads/{}", branch);
        let mut refn = repo.find_reference(&ref_name)?;
        refn.set_target(upstream, "nog: fast-forward")?;
    }
    {
        let commit = repo.find_commit(upstream)?;
        let tree = commit.tree()?;
        repo.checkout_tree(&tree.into_object(), Some(CheckoutBuilder::new().force()))?;
    }

    let kept = pop_stash_best_effort(repo, stashed);
    Ok(SyncOutcome {
        result: if kept {
            SyncResult::StashKept
        } else {
            SyncResult::Synced
        },
        message: if kept {
            "Fast-forwarded; local changes conflict with the result, stash kept".to_string()
        } else {
            format!("Fast-forwarded '{}' to its upstream", branch)
        },
        pre_merge_head: None,
        conflicted_files: Vec::new(),
        stashed: false,
    })
}

fn sync_merge_commit(
    repo: &mut Repository,
    branch: &str,
    pre_merge_head: Oid,
    upstream: Oid,
    stashed: bool,
) -> Result<SyncOutcome> {
    let analysis = {
        let their = repo.find_annotated_commit(upstream)?;
        let (analysis, _preference) = repo.merge_analysis(&[&their])?;
        analysis
    };

    if analysis.is_up_to_date() {
        pop_stash_if_needed(repo, stashed)?;
        return Ok(SyncOutcome {
            result: SyncResult::UpToDate,
            message: format!("'{}' is up to date with its upstream", branch),
            pre_merge_head: None,
            conflicted_files: Vec::new(),
            stashed: false,
        });
    }

    // 3-way merge into the index and working tree. `fail_on_conflict` is false
    // so a content conflict leaves a resumable state instead of erroring.
    {
        let their = repo.find_annotated_commit(upstream)?;
        let mut merge_opts = git2::MergeOptions::new();
        merge_opts.fail_on_conflict(false);
        repo.merge(&[&their], Some(&mut merge_opts), None)?;
    }

    let conflicted = status::unmerged_files(repo)?;
    if !conflicted.is_empty() {
        // Leave the merge in progress for the conflict view. Do not pop the
        // stash yet; finalize/abort handle it.
        return Ok(SyncOutcome {
            result: SyncResult::Conflicted,
            message: format!("Merge conflicted in {} file(s)", conflicted.len()),
            pre_merge_head: Some(pre_merge_head),
            conflicted_files: conflicted,
            stashed,
        });
    }

    // Clean merge: create the two-parent merge commit and clear merge state.
    commit_merge(repo, pre_merge_head, upstream, branch)?;
    repo.cleanup_state()?;

    let kept = pop_stash_best_effort(repo, stashed);
    Ok(SyncOutcome {
        result: if kept {
            SyncResult::StashKept
        } else {
            SyncResult::Synced
        },
        message: if kept {
            "Merged; local changes conflict with the result, stash kept".to_string()
        } else {
            format!("Merged '{}' into '{}'", short(&upstream), branch)
        },
        pre_merge_head: None,
        conflicted_files: Vec::new(),
        stashed: false,
    })
}

/// Create a two-parent merge commit from the current index.
fn commit_merge(
    repo: &mut Repository,
    pre_merge_head: Oid,
    upstream: Oid,
    branch: &str,
) -> Result<()> {
    let head_commit = repo.find_commit(pre_merge_head)?;
    let their_commit = repo.find_commit(upstream)?;
    let tree_oid = repo.index()?.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    let sig = repo.signature()?;
    let msg = format!("Merge commit '{}' into '{}'", short(&upstream), branch);
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &msg,
        &tree,
        &[&head_commit, &their_commit],
    )?;
    Ok(())
}

/// Finalize an in-progress merge: verify no conflicts remain, create the
/// merge commit, clear merge state, and re-apply any stash.
pub fn finalize_merge(repo_path: &Path, pre_merge_head: Oid, stashed: bool) -> Result<SyncOutcome> {
    let mut repo = Repository::open(repo_path)?;
    let branch = current_branch_name(&repo)?;

    let conflicted = status::unmerged_files(&repo)?;
    if !conflicted.is_empty() {
        return Ok(SyncOutcome {
            result: SyncResult::Conflicted,
            message: format!(
                "{} conflict(s) remain; resolve before finalizing",
                conflicted.len()
            ),
            pre_merge_head: Some(pre_merge_head),
            conflicted_files: conflicted,
            stashed,
        });
    }

    // The upstream is recorded in MERGE_HEAD by the in-progress merge.
    let upstream = repo
        .find_reference("MERGE_HEAD")?
        .target()
        .ok_or_else(|| anyhow::anyhow!("no MERGE_HEAD; nothing to finalize"))?;

    commit_merge(&mut repo, pre_merge_head, upstream, &branch)?;
    repo.cleanup_state()?;

    let kept = pop_stash_best_effort(&mut repo, stashed);
    Ok(SyncOutcome {
        result: if kept {
            SyncResult::StashKept
        } else {
            SyncResult::Synced
        },
        message: if kept {
            "Merged; local changes conflict with the result, stash kept".to_string()
        } else {
            format!("Merge committed into '{}'", branch)
        },
        pre_merge_head: None,
        conflicted_files: Vec::new(),
        stashed: false,
    })
}

/// Abort an in-progress merge: restore the pre-merge HEAD (index + working
/// tree) and clear merge state, then re-apply the stash onto the restored HEAD.
pub fn abort_merge(repo_path: &Path, pre_merge_head: Oid, stashed: bool) -> Result<SyncOutcome> {
    let mut repo = Repository::open(repo_path)?;

    {
        let head = repo.find_commit(pre_merge_head)?;
        repo.reset(head.as_object(), ResetType::Hard, None)?;
    }
    repo.cleanup_state()?;

    // Restore the stashed local changes onto the original HEAD.
    let kept = pop_stash_best_effort(&mut repo, stashed);
    Ok(SyncOutcome {
        result: if kept {
            SyncResult::StashKept
        } else {
            SyncResult::Synced
        },
        message: if kept {
            "Aborted; local changes conflict, stash kept".to_string()
        } else {
            "Pull aborted; original state restored".to_string()
        },
        pre_merge_head: None,
        conflicted_files: Vec::new(),
        stashed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn git(args: &[&str], cwd: &std::path::Path) {
        let status = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .expect("git not found or failed");
        assert!(status.success(), "git {:?} failed in {:?}", args, cwd);
    }

    fn set_user(cwd: &std::path::Path) {
        git(&["config", "user.email", "t@example.com"], cwd);
        git(&["config", "user.name", "Tester"], cwd);
    }

    /// A local repo with a bare `origin` remote at a shared base commit.
    fn local_with_remote(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("nog_sync_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
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
        set_user(&local);

        // Base commit, tracked on origin.
        std::fs::write(local.join("file.txt"), "base").unwrap();
        git(&["add", "file.txt"], &local);
        git(&["commit", "-m", "base"], &local);
        git(&["push", "-u", "origin", "HEAD"], &local);
        local
    }

    /// Push a new commit to the bare `origin` remote that local does not yet
    /// have.  A throwaway fork clones the *bare* remote (so the new commit
    /// lands where local's fetch can see it, not back into local's worktree).
    fn advance_remote(local: &std::path::Path, file: &str, contents: &str, msg: &str) {
        let base = local.parent().unwrap();
        let remote = base.join("remote.git");
        let fork = base.join("fork");
        let _ = std::fs::remove_dir_all(&fork);
        git(
            &["clone", remote.to_str().unwrap(), fork.to_str().unwrap()],
            base,
        );
        set_user(&fork);
        std::fs::write(fork.join(file), contents).unwrap();
        git(&["add", "."], &fork);
        git(&["commit", "-m", msg], &fork);
        git(&["push", "origin", "HEAD"], &fork);
    }

    #[test]
    fn fast_forward_sync_restores_stashed_local_change() {
        let local = local_with_remote("ff");
        // Remote gains an unrelated commit; local has an uncommitted change to a
        // different tracked file.
        advance_remote(&local, "readme.md", "docs", "remote commit");
        std::fs::write(local.join("file.txt"), "local-mod").unwrap();

        let outcome = sync(&local, PullMode::FastForwardOnly).expect("sync failed");
        assert_eq!(outcome.result, SyncResult::Synced);

        // The fast-forward applied and the stashed local change was popped back.
        assert_eq!(
            std::fs::read_to_string(local.join("file.txt")).unwrap(),
            "local-mod"
        );
        assert!(local.join("readme.md").exists());
    }

    #[test]
    fn merge_conflict_is_detected_and_can_finalize() {
        let local = local_with_remote("conflict");
        // Diverge: local and remote both change the same file off the base.
        std::fs::write(local.join("file.txt"), "local").unwrap();
        git(&["add", "file.txt"], &local);
        git(&["commit", "-m", "local commit"], &local);
        advance_remote(&local, "file.txt", "remote", "remote commit");

        let outcome = sync(&local, PullMode::MergeCommit).expect("sync failed");
        assert_eq!(outcome.result, SyncResult::Conflicted);
        assert!(
            outcome.conflicted_files.iter().any(|f| f == "file.txt"),
            "file.txt should be listed as conflicted: {:?}",
            outcome.conflicted_files
        );
        let pre_merge_head = outcome.pre_merge_head.expect("pre-merge head recorded");

        // Resolve the conflict and finalize the merge.
        std::fs::write(local.join("file.txt"), "merged").unwrap();
        git(&["add", "file.txt"], &local);
        let finalized = finalize_merge(&local, pre_merge_head, false).expect("finalize failed");
        assert_eq!(finalized.result, SyncResult::Synced);
    }

    #[test]
    fn abort_merge_restores_original_head() {
        let local = local_with_remote("abort");
        std::fs::write(local.join("file.txt"), "local").unwrap();
        git(&["add", "file.txt"], &local);
        git(&["commit", "-m", "local commit"], &local);
        advance_remote(&local, "file.txt", "remote", "remote commit");

        let outcome = sync(&local, PullMode::MergeCommit).expect("sync failed");
        assert_eq!(outcome.result, SyncResult::Conflicted);
        let pre_merge_head = outcome.pre_merge_head.expect("pre-merge head recorded");

        let aborted = abort_merge(&local, pre_merge_head, false).expect("abort failed");
        assert_eq!(aborted.result, SyncResult::Synced);

        // The merge is undone: no conflict markers, original file content intact.
        let content = std::fs::read_to_string(local.join("file.txt")).unwrap();
        assert!(!content.contains("<<<<<<<"), "conflict marker left behind");
        assert_eq!(content, "local");
        let repo = git2::Repository::open(&local).unwrap();
        let head = repo.head().unwrap().target().unwrap();
        assert_eq!(
            head, pre_merge_head,
            "HEAD should be restored to pre-merge state"
        );
    }
}
