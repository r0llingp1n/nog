use git2::Oid;

/// One row of the computed graph layout, corresponding to one commit.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GraphRow {
    pub oid: Oid,
    /// Column index where the commit node sits
    pub commit_col: usize,
    /// Total number of active columns in this row
    pub num_cols: usize,
    /// Which columns have pass-through pipes and their color indices
    pub active_cols: Vec<Option<usize>>,
    /// Merge lines coming from the right (parent columns > commit_col)
    pub merge_from: Vec<(usize, usize)>, // (column, color_index)
    /// Fork lines going to the left (parent columns < commit_col)
    pub fork_to: Vec<(usize, usize)>, // (column, color_index)
    /// Color index for the commit itself
    pub commit_color: usize,
}
