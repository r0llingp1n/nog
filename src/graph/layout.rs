use std::collections::HashMap;

use git2::Oid;

use crate::git::types::CommitInfo;

use super::types::GraphRow;

const NUM_COLORS: usize = 8;

/// Compute the graph layout for a list of commits in topological order.
///
/// Returns one GraphRow per commit with column assignments and pipe information.
pub fn compute_graph_layout(commits: &[CommitInfo]) -> Vec<GraphRow> {
    let mut rows = Vec::with_capacity(commits.len());

    // Active columns: each entry tracks which OID a column is "waiting for".
    // None means the column is free.
    let mut columns: Vec<Option<Oid>> = Vec::new();

    // Color assignment per OID
    let mut color_map: HashMap<Oid, usize> = HashMap::new();
    let mut next_color: usize = 0;

    for commit in commits {
        let oid = commit.oid;

        // Step 1: Find the column for this commit
        let commit_col = match columns.iter().position(|c| *c == Some(oid)) {
            Some(col) => {
                columns[col] = None; // Free it, we'll reassign below
                col
            }
            None => {
                // New branch head — find first free slot or append
                match columns.iter().position(|c| c.is_none()) {
                    Some(col) => col,
                    None => {
                        columns.push(None);
                        columns.len() - 1
                    }
                }
            }
        };

        // Assign color for this commit if not already assigned
        let commit_color = *color_map.entry(oid).or_insert_with(|| {
            let c = next_color;
            next_color = (next_color + 1) % NUM_COLORS;
            c
        });

        // Step 2: Process parents
        let mut merge_from = Vec::new();
        let mut fork_to = Vec::new();

        for (i, &parent_oid) in commit.parent_oids.iter().enumerate() {
            if i == 0 {
                // First parent: takes the commit's column (keeps the line straight)
                columns[commit_col] = Some(parent_oid);
                // Inherit color
                color_map.entry(parent_oid).or_insert(commit_color);
            } else {
                // Merge parent: check if already tracked
                let existing = columns.iter().position(|c| *c == Some(parent_oid));
                match existing {
                    Some(col) => {
                        // Already tracked in another column — record merge line
                        let color = *color_map.entry(parent_oid).or_insert_with(|| {
                            let c = next_color;
                            next_color = (next_color + 1) % NUM_COLORS;
                            c
                        });
                        if col > commit_col {
                            merge_from.push((col, color));
                        } else {
                            fork_to.push((col, color));
                        }
                    }
                    None => {
                        // Need a new column for this parent
                        let color = {
                            let c = next_color;
                            next_color = (next_color + 1) % NUM_COLORS;
                            c
                        };
                        color_map.insert(parent_oid, color);

                        // Find first free slot or append
                        let col = match columns.iter().position(|c| c.is_none()) {
                            Some(col) => col,
                            None => {
                                columns.push(None);
                                columns.len() - 1
                            }
                        };
                        columns[col] = Some(parent_oid);
                        if col > commit_col {
                            merge_from.push((col, color));
                        } else {
                            fork_to.push((col, color));
                        }
                    }
                }
            }
        }

        // If commit has no parents (root), column stays None (freed)
        if commit.parent_oids.is_empty() {
            columns[commit_col] = None;
        }

        // Build active_cols snapshot (for rendering pass-through pipes)
        let active_cols: Vec<Option<usize>> = columns
            .iter()
            .map(|slot| slot.and_then(|oid| color_map.get(&oid).copied()))
            .collect();

        let num_cols = columns.len();

        rows.push(GraphRow {
            oid,
            commit_col,
            num_cols,
            active_cols,
            merge_from,
            fork_to,
            commit_color,
        });

        // Step 3: Compact — trim trailing None columns
        while columns.last() == Some(&None) {
            columns.pop();
        }
    }

    rows
}
