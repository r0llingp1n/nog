use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use git2::{BranchType, Oid, Repository};

use super::types::{BranchInfo, Decorations, TagInfo};

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
                map.entry(commit.id()).or_default().tags.push(TagInfo { name });
            }
        }
        true
    })?;

    Ok(map)
}
