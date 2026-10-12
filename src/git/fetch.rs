use anyhow::Result;
use git2::{FetchOptions, FetchPrune, Repository};

/// Fetch every configured remote with pruning.
///
/// This is the in-process (libgit2) equivalent of `git fetch --all --prune`:
/// it iterates all remotes and, for each, fetches its configured refspecs
/// while pruning stale remote-tracking refs. No external process is spawned.
pub fn fetch_all_prune(repo: &Repository) -> Result<()> {
    let remotes = repo.remotes()?;
    for name in remotes.iter().flatten() {
        let mut remote = repo.find_remote(name)?;
        let mut opts = FetchOptions::new();
        opts.prune(FetchPrune::On);
        // An empty refspec list makes libgit2 use the remote's configured
        // fetch refspecs (all branches), matching `git fetch <remote>`.
        remote.fetch(&[] as &[&str], Some(&mut opts), None)?;
    }
    Ok(())
}
