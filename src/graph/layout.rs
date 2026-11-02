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

        // Step 1: Find the column(s) for this commit.
        // Several columns can be waiting for the same OID when multiple
        // branches share it as their next (first) parent — e.g. the root
        // commit, which every lane eventually descends to. The lowest such
        // column becomes the commit's column; the rest collapse into it.
        let matching_cols: Vec<usize> = columns
            .iter()
            .enumerate()
            .filter_map(|(i, c)| if *c == Some(oid) { Some(i) } else { None })
            .collect();

        let commit_col = match matching_cols.first() {
            Some(&col) => col,
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

        // Free every column that was waiting for this commit; we'll reassign
        // commit_col below and the extras collapse in as connectors.
        for &col in &matching_cols {
            columns[col] = None;
        }

        // Assign color for this commit if not already assigned
        let commit_color = *color_map.entry(oid).or_insert_with(|| {
            let c = next_color;
            next_color = (next_color + 1) % NUM_COLORS;
            c
        });

        // Step 2: Process parents
        let mut merge_from = Vec::new();
        let mut fork_to = Vec::new();

        // Columns besides commit_col that were waiting for this commit collapse
        // into it. Record a connector for each so they join the commit's column
        // instead of dangling as bare vertical pipes (notably on the root row).
        for &col in matching_cols.iter().skip(1) {
            if col > commit_col {
                merge_from.push((col, commit_color));
            } else {
                fork_to.push((col, commit_color));
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    fn commit(oid: Oid, parents: Vec<Oid>) -> CommitInfo {
        CommitInfo {
            oid,
            short_id: format!("{:.7}", oid),
            summary: String::new(),
            author_name: String::new(),
            author_date: Local::now(),
            parent_oids: parents,
        }
    }

    fn oid(byte: u8) -> Oid {
        Oid::from_bytes(&[byte; 20]).unwrap()
    }

    /// Regression: several branches fork off the root commit, so on the final
    /// (root) row every lane is waiting for the root's OID. The root must claim
    /// one column and the rest must collapse into it as connectors, rather than
    /// surviving as free-floating vertical pipes.
    #[test]
    fn root_row_collapses_forked_lanes_into_connectors() {
        let root = oid(0);
        // Three branch tips, each whose (only) parent is the root commit.
        let tip_a = oid(1);
        let tip_b = oid(2);
        let tip_c = oid(3);

        // Topological order: tips first (newest), root last (oldest).
        let commits = vec![
            commit(tip_a, vec![root]),
            commit(tip_b, vec![root]),
            commit(tip_c, vec![root]),
            commit(root, vec![]),
        ];

        let rows = compute_graph_layout(&commits);
        let root_row = rows.last().expect("root row");

        assert_eq!(root_row.oid, root);
        // Root sits in the leftmost lane.
        assert_eq!(root_row.commit_col, 0);

        // The two other lanes that were waiting for the root collapse into it as
        // connectors (merge_from, since they sit to the right of column 0),
        // instead of lingering as bare active pipes.
        assert_eq!(
            root_row.merge_from.len(),
            2,
            "both forked lanes should join the root: {root_row:?}"
        );
        let mut connector_cols: Vec<usize> = root_row.merge_from.iter().map(|&(c, _)| c).collect();
        connector_cols.sort_unstable();
        assert_eq!(connector_cols, vec![1, 2]);

        // No lane other than the root's own column remains active on the root row.
        assert!(
            root_row.active_cols.iter().all(|c| c.is_none()),
            "no dangling pipes should remain on the root row: {root_row:?}"
        );
    }

    /// Interior rows with a single merge parent keep their existing shape — a
    /// lone merge connector and no spurious collapse.
    #[test]
    fn interior_merge_row_unaffected() {
        let a = oid(10);
        let b = oid(11);
        let base = oid(12);

        // `a` is a merge of `base` (first parent) and `b` (second parent).
        let commits = vec![
            commit(a, vec![base, b]),
            commit(b, vec![base]),
            commit(base, vec![]),
        ];

        let rows = compute_graph_layout(&commits);
        let merge_row = &rows[0];
        assert_eq!(merge_row.oid, a);
        // One merge connector for the second parent `b`.
        assert_eq!(merge_row.merge_from.len() + merge_row.fork_to.len(), 1);
    }
}
