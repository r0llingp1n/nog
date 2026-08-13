use anyhow::Result;
use chrono::{Local, TimeZone};
use git2::{Repository, Sort};

use super::types::CommitInfo;

pub fn walk_commits(repo: &Repository, max_count: usize) -> Result<Vec<CommitInfo>> {
    let mut revwalk = repo.revwalk()?;

    // Push all branch heads so we see the full graph
    for branch in repo.branches(None)? {
        let (branch, _) = branch?;
        if let Some(oid) = branch.get().target() {
            revwalk.push(oid)?;
        }
    }

    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    let mut commits = Vec::new();
    for oid_result in revwalk {
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        let short_id = format!("{:.7}", oid);
        let summary = commit.summary().unwrap_or("").to_string();
        let author = commit.author();
        let author_name = author.name().unwrap_or("").to_string();
        let author_date = Local
            .timestamp_opt(author.when().seconds(), 0)
            .single()
            .unwrap_or_else(Local::now);
        let parent_oids: Vec<_> = (0..commit.parent_count())
            .filter_map(|i| commit.parent_id(i).ok())
            .collect();

        commits.push(CommitInfo {
            oid,
            short_id,
            summary,
            author_name,
            author_date,
            parent_oids,
        });

        if commits.len() >= max_count {
            break;
        }
    }

    Ok(commits)
}
