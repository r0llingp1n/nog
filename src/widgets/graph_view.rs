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
    pub show_remotes: bool,
}

impl<'a> GraphView<'a> {
    /// Get the display label for a commit's refs (branches + tags).
    fn ref_label(&self, oid: &Oid) -> Option<(String, ratatui::style::Color)> {
        let d = self.decorations.get(oid)?;

        // Pick the most important ref to display: HEAD branch > local branch > remote > tag
        for branch in &d.branches {
            if branch.is_head {
                return Some((
                    format!("HEAD -> {}", branch.name),
                    ratatui::style::Color::Green,
                ));
            }
        }
        for branch in &d.branches {
            if !branch.is_remote {
                return Some((branch.name.clone(), ratatui::style::Color::Green));
            }
        }
        if self.show_remotes {
            for branch in &d.branches {
                if branch.is_remote {
                    return Some((branch.name.clone(), ratatui::style::Color::Red));
                }
            }
        }
        for tag in &d.tags {
            return Some((tag.name.clone(), ratatui::style::Color::Yellow));
        }

        None
    }

    /// Get all ref labels for a commit (for the extra labels after the primary one).
    fn extra_ref_labels(&self, oid: &Oid) -> Vec<(String, ratatui::style::Color)> {
        let d = match self.decorations.get(oid) {
            Some(d) => d,
            None => return Vec::new(),
        };

        let mut labels = Vec::new();
        let mut first_skipped = false;

        // Same priority order as ref_label — skip the first match
        for branch in &d.branches {
            if branch.is_head {
                if !first_skipped {
                    first_skipped = true;
                    continue;
                }
                labels.push((
                    format!("HEAD -> {}", branch.name),
                    ratatui::style::Color::Green,
                ));
            }
        }
        for branch in &d.branches {
            if !branch.is_remote && !branch.is_head {
                if !first_skipped {
                    first_skipped = true;
                    continue;
                }
                labels.push((branch.name.clone(), ratatui::style::Color::Green));
            }
        }
        if self.show_remotes {
            for branch in &d.branches {
                if branch.is_remote {
                    if !first_skipped {
                        first_skipped = true;
                        continue;
                    }
                    labels.push((branch.name.clone(), ratatui::style::Color::Red));
                }
            }
        }
        for tag in &d.tags {
            if !first_skipped {
                first_skipped = true;
                continue;
            }
            labels.push((tag.name.clone(), ratatui::style::Color::Yellow));
        }

        labels
    }
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

        let visible_start = state.offset;
        let visible_end = (state.offset + height).min(self.commits.len());

        // Find max graph width for alignment
        let max_cols = self
            .graph_rows
            .iter()
            .skip(visible_start)
            .take(height)
            .map(|r| r.num_cols)
            .max()
            .unwrap_or(1);
        let graph_width = max_cols * 2;

        // Find max ref label width across visible rows for alignment
        let max_ref_width = (visible_start..visible_end)
            .filter_map(|i| {
                self.ref_label(&self.commits[i].oid)
                    .map(|(label, _)| label.len())
            })
            .max()
            .unwrap_or(0);

        // ref column width: label + connecting chars "── label ─"
        // 0 if no refs visible
        let ref_col_width = if max_ref_width > 0 {
            max_ref_width + 4 // "── " + label + " "
        } else {
            0
        };

        for (screen_row, idx) in (visible_start..visible_end).enumerate() {
            let commit = &self.commits[idx];
            let graph_row = &self.graph_rows[idx];
            let is_selected = idx == state.selected;

            let line = self.render_row(
                commit,
                graph_row,
                graph_width,
                ref_col_width,
                max_ref_width,
                area.width as usize,
            );

            let y = area.y + screen_row as u16;

            let bg_style = if is_selected {
                Style::default().bg(theme::SELECTED_BG)
            } else {
                Style::default()
            };

            // Clear the entire row first
            for cx in area.x..area.x + area.width {
                buf.cell_mut((cx, y))
                    .map(|cell| cell.set_char(' ').set_style(bg_style));
            }

            // Render the line spans
            let mut x = area.x;
            for span in line.spans.iter() {
                let style = if is_selected {
                    span.style.bg(theme::SELECTED_BG)
                } else {
                    span.style
                };
                for ch in span.content.chars() {
                    if x >= area.x + area.width {
                        break;
                    }
                    let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                    buf.cell_mut((x, y))
                        .map(|cell| cell.set_char(ch).set_style(style));
                    x += w as u16;
                }
                if x >= area.x + area.width {
                    break;
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
        ref_col_width: usize,
        max_ref_width: usize,
        total_width: usize,
    ) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();

        // Build a set of merge/fork target columns for horizontal line drawing
        let mut merge_cols: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut merge_color_map: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for &(col, color) in &graph_row.merge_from {
            merge_cols.insert(col);
            merge_color_map.insert(col, color);
        }
        for &(col, color) in &graph_row.fork_to {
            merge_cols.insert(col);
            merge_color_map.insert(col, color);
        }

        let merge_min = merge_cols.iter().copied().min();
        let merge_max = merge_cols.iter().copied().max();
        let has_merge = !merge_cols.is_empty();

        // Render graph columns
        let mut graph_chars = 0;
        for col in 0..graph_row.num_cols {
            if col == graph_row.commit_col {
                spans.push(Span::styled(
                    glyphs::COMMIT_NODE.to_string(),
                    Style::default().fg(theme::graph_color(graph_row.commit_color)),
                ));
                let needs_line = has_merge
                    && merge_min.is_some()
                    && merge_max.is_some()
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
                let color = merge_color_map[&col];
                spans.push(Span::styled(
                    glyphs::VERTICAL.to_string(),
                    Style::default().fg(theme::graph_color(color)),
                ));
                spans.push(Span::raw(" ".to_string()));
            } else if let Some(Some(color_idx)) = graph_row.active_cols.get(col) {
                spans.push(Span::styled(
                    glyphs::VERTICAL.to_string(),
                    Style::default().fg(theme::graph_color(*color_idx)),
                ));
                let between_merge = has_merge
                    && merge_min.is_some()
                    && merge_max.is_some()
                    && {
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
                let between_merge = has_merge
                    && merge_min.is_some()
                    && merge_max.is_some()
                    && {
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

        // Ref label column with connecting line
        if ref_col_width > 0 {
            let primary = self.ref_label(&commit.oid);
            let extras = self.extra_ref_labels(&commit.oid);

            if let Some((label, color)) = primary {
                // ── label ──  (padded to fixed width)
                let commit_color = theme::graph_color(graph_row.commit_color);
                spans.push(Span::styled(
                    "──".to_string(),
                    Style::default().fg(commit_color),
                ));
                spans.push(Span::styled(
                    " ".to_string(),
                    Style::default(),
                ));
                spans.push(Span::styled(
                    label.clone(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));

                // Extra refs inline after primary
                for (extra_label, extra_color) in &extras {
                    spans.push(Span::styled(
                        ", ".to_string(),
                        Style::default().fg(theme::DATE_COLOR),
                    ));
                    spans.push(Span::styled(
                        extra_label.clone(),
                        Style::default()
                            .fg(*extra_color)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                // Pad remaining ref column
                let extras_len: usize = extras
                    .iter()
                    .map(|(l, _)| l.len() + 2) // ", " + label
                    .sum();
                let label_used = label.len() + extras_len;
                let pad = max_ref_width.saturating_sub(label_used);
                if pad > 0 {
                    spans.push(Span::raw(" ".repeat(pad)));
                }
                spans.push(Span::raw(" ".to_string()));
            } else {
                // No ref — fill with spaces to keep alignment
                spans.push(Span::raw(" ".repeat(ref_col_width)));
            }
        } else {
            spans.push(Span::raw(" ".to_string()));
        }

        // Short hash
        spans.push(Span::styled(
            commit.short_id.clone(),
            Style::default().fg(theme::HASH_COLOR),
        ));
        spans.push(Span::raw(" ".to_string()));

        // Summary — truncate to fit, leaving room for author and date
        let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let date_str = relative_time(commit.author_date);
        let author_str = &commit.author_name;
        let right_width = date_str.chars().count() + 2 + author_str.chars().count() + 2;
        let remaining = total_width.saturating_sub(used + right_width);

        let summary = if commit.summary.chars().count() > remaining && remaining > 3 {
            let truncated: String = commit.summary.chars().take(remaining - 1).collect();
            format!("{}…", truncated)
        } else {
            commit.summary.clone()
        };
        spans.push(Span::raw(summary.clone()));

        // Right-align author + date
        let after_summary = used + summary.chars().count();
        let padding = total_width.saturating_sub(after_summary + right_width);
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding)));
        }
        spans.push(Span::raw(" ".to_string()));
        spans.push(Span::styled(
            author_str.clone(),
            Style::default().fg(theme::AUTHOR_COLOR),
        ));
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
