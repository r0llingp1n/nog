use chrono::Local;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::StatefulWidget;

use crate::git::types::{CommitInfo, Decorations};
use crate::graph::glyphs;
use crate::graph::types::GraphRow;
use crate::theme;

use std::collections::HashMap;
use git2::Oid;

pub struct GraphViewState {
    pub selected: usize,
    pub offset: usize,
}

impl GraphViewState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            offset: 0,
        }
    }
}

pub struct GraphView<'a> {
    pub commits: &'a [CommitInfo],
    pub graph_rows: &'a [GraphRow],
    pub decorations: &'a HashMap<Oid, Decorations>,
}

impl<'a> StatefulWidget for GraphView<'a> {
    type State = GraphViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let height = area.height as usize;
        if height == 0 || self.commits.is_empty() {
            return;
        }

        // Adjust offset to keep selection visible
        if state.selected < state.offset {
            state.offset = state.selected;
        }
        if state.selected >= state.offset + height {
            state.offset = state.selected - height + 1;
        }

        // Find max graph width for alignment
        let max_cols = self
            .graph_rows
            .iter()
            .skip(state.offset)
            .take(height)
            .map(|r| r.num_cols)
            .max()
            .unwrap_or(1);
        let graph_width = max_cols * 2;

        for (screen_row, idx) in (state.offset..state.offset + height).enumerate() {
            if idx >= self.commits.len() {
                break;
            }

            let commit = &self.commits[idx];
            let graph_row = &self.graph_rows[idx];
            let is_selected = idx == state.selected;

            let line = self.render_row(commit, graph_row, graph_width, area.width as usize);

            let y = area.y + screen_row as u16;

            // Render the line into the buffer
            let mut x = area.x;
            for span in line.spans.iter() {
                let style = if is_selected {
                    span.style.bg(theme::SELECTED_BG)
                } else {
                    span.style
                };
                let content = &span.content;
                for ch in content.chars() {
                    if x >= area.x + area.width {
                        break;
                    }
                    buf.cell_mut((x, y))
                        .map(|cell| cell.set_char(ch).set_style(style));
                    x += 1;
                }
            }

            // Fill remaining width with selected bg if selected
            if is_selected {
                while x < area.x + area.width {
                    buf.cell_mut((x, y))
                        .map(|cell| cell.set_style(Style::default().bg(theme::SELECTED_BG)));
                    x += 1;
                }
            }
        }
    }
}

impl<'a> GraphView<'a> {
    fn render_row(
        &self,
        commit: &CommitInfo,
        graph_row: &GraphRow,
        graph_width: usize,
        total_width: usize,
    ) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();

        // Build a set of merge/fork target columns for horizontal line drawing
        let mut merge_cols: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut merge_color_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &(col, color) in &graph_row.merge_from {
            merge_cols.insert(col);
            merge_color_map.insert(col, color);
        }
        for &(col, color) in &graph_row.fork_to {
            merge_cols.insert(col);
            merge_color_map.insert(col, color);
        }

        // Determine the range of merge lines
        let merge_min = merge_cols.iter().copied().min();
        let merge_max = merge_cols.iter().copied().max();
        let has_merge = !merge_cols.is_empty();

        // Render graph columns
        let mut graph_chars = 0;
        for col in 0..graph_row.num_cols {
            if col == graph_row.commit_col {
                // Commit node
                spans.push(Span::styled(
                    glyphs::COMMIT_NODE.to_string(),
                    Style::default().fg(theme::graph_color(graph_row.commit_color)),
                ));
                // Draw horizontal line toward merge columns if needed
                let needs_line = has_merge && merge_min.is_some() && merge_max.is_some()
                    && ((col < merge_max.unwrap() && col >= graph_row.commit_col)
                        || (col > merge_min.unwrap() && col <= graph_row.commit_col));
                if needs_line {
                    spans.push(Span::styled(
                        "─".to_string(),
                        Style::default().fg(theme::graph_color(graph_row.commit_color)),
                    ));
                } else {
                    spans.push(Span::raw(" ".to_string()));
                }
            } else if merge_cols.contains(&col) {
                // Merge/fork endpoint
                let color = merge_color_map[&col];
                if let Some(Some(_active_color)) = graph_row.active_cols.get(col) {
                    // Column has a pass-through AND is a merge target
                    spans.push(Span::styled(
                        glyphs::VERTICAL.to_string(),
                        Style::default().fg(theme::graph_color(color)),
                    ));
                } else {
                    spans.push(Span::styled(
                        glyphs::VERTICAL.to_string(),
                        Style::default().fg(theme::graph_color(color)),
                    ));
                }
                spans.push(Span::raw(" ".to_string()));
            } else if let Some(Some(color_idx)) = graph_row.active_cols.get(col) {
                // Pass-through pipe
                spans.push(Span::styled(
                    glyphs::VERTICAL.to_string(),
                    Style::default().fg(theme::graph_color(*color_idx)),
                ));
                // Draw horizontal merge line through if this column is between commit and merge target
                let between_merge = has_merge && merge_min.is_some() && merge_max.is_some() && {
                    let lo = graph_row.commit_col.min(merge_min.unwrap());
                    let hi = graph_row.commit_col.max(merge_max.unwrap());
                    col > lo && col < hi
                };
                if between_merge {
                    spans.push(Span::styled(
                        "─".to_string(),
                        Style::default().fg(theme::graph_color(graph_row.commit_color)),
                    ));
                } else {
                    spans.push(Span::raw(" ".to_string()));
                }
            } else {
                // Empty column — but may need horizontal merge line through it
                let between_merge = has_merge && merge_min.is_some() && merge_max.is_some() && {
                    let lo = graph_row.commit_col.min(merge_min.unwrap());
                    let hi = graph_row.commit_col.max(merge_max.unwrap());
                    col > lo && col < hi
                };
                if between_merge {
                    spans.push(Span::styled(
                        "──".to_string(),
                        Style::default().fg(theme::graph_color(graph_row.commit_color)),
                    ));
                } else {
                    spans.push(Span::raw("  ".to_string()));
                }
            }
            graph_chars += 2;
        }

        // Pad graph to fixed width
        if graph_chars < graph_width {
            spans.push(Span::raw(" ".repeat(graph_width - graph_chars)));
        }

        // Separator
        spans.push(Span::raw(" ".to_string()));

        // Short hash
        spans.push(Span::styled(
            commit.short_id.clone(),
            Style::default().fg(theme::HASH_COLOR),
        ));
        spans.push(Span::raw(" ".to_string()));

        // Decorations
        let decos = self.decorations.get(&commit.oid);
        if let Some(d) = decos {
            for branch in &d.branches {
                if branch.is_remote {
                    continue; // Skip remote branches in the compact view
                }
                let (label, color) = if branch.is_head {
                    (
                        format!("HEAD -> {}", branch.name),
                        ratatui::style::Color::Green,
                    )
                } else {
                    (branch.name.clone(), ratatui::style::Color::Green)
                };
                spans.push(Span::styled(
                    format!("({}) ", label),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            for tag in &d.tags {
                spans.push(Span::styled(
                    format!("[{}] ", tag.name),
                    Style::default()
                        .fg(ratatui::style::Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        }

        // Summary — truncate to fit
        let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let date_str = relative_time(commit.author_date);
        let date_width = date_str.chars().count() + 2; // 2 for spacing
        let remaining = total_width.saturating_sub(used + date_width);

        let summary = if commit.summary.chars().count() > remaining && remaining > 3 {
            let truncated: String = commit.summary.chars().take(remaining - 1).collect();
            format!("{}…", truncated)
        } else {
            commit.summary.clone()
        };
        spans.push(Span::raw(summary.clone()));

        // Right-align date
        let after_summary = used + summary.chars().count();
        let padding = total_width.saturating_sub(after_summary + date_width);
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding)));
        }
        spans.push(Span::raw(" ".to_string()));
        spans.push(Span::styled(
            date_str,
            Style::default().fg(theme::DATE_COLOR),
        ));

        Line::from(spans)
    }
}

fn relative_time(dt: chrono::DateTime<Local>) -> String {
    let now = Local::now();
    let duration = now.signed_duration_since(dt);

    let secs = duration.num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{}m ago", mins);
    }
    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{}h ago", hours);
    }
    let days = duration.num_days();
    if days < 30 {
        return format!("{}d ago", days);
    }
    let months = days / 30;
    if months < 12 {
        return format!("{}mo ago", months);
    }
    let years = days / 365;
    format!("{}y ago", years)
}
