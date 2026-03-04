use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::app::StashViewFocus;
use crate::git::diff::DiffData;
use crate::git::stash::StashEntry;
use crate::git::status::{FileStatus, FileStatusCode};
use crate::theme;

pub struct StashView<'a> {
    pub entries: &'a [StashEntry],
    pub selected: usize,
    pub focus: StashViewFocus,
    pub files: &'a [FileStatus],
    pub file_selected: usize,
    pub diff: Option<&'a DiffData>,
    pub diff_scroll: u16,
    pub diff_path: &'a str,
    pub error: Option<&'a str>,
}

impl StashView<'_> {
    fn render_stash_list(&self, area: Rect, buf: &mut Buffer) {
        let focused = self.focus == StashViewFocus::StashList;
        let border_color = if focused {
            theme::FOCUS_BORDER
        } else {
            theme::UNFOCUS_BORDER
        };

        let title_style = if focused {
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(border_color)
        };

        let block = Block::default()
            .title(Span::styled(
                format!(" Stash Stack ({}) ", self.entries.len()),
                title_style,
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.entries.is_empty() {
            let empty = Paragraph::new(Span::styled(
                "(no stashes)",
                Style::default().fg(ratatui::style::Color::DarkGray),
            ));
            empty.render(inner, buf);
            return;
        }

        let visible_height = inner.height as usize;
        let offset = if self.selected >= visible_height {
            self.selected - visible_height + 1
        } else {
            0
        };

        let mut lines: Vec<Line<'static>> = Vec::new();
        for (i, entry) in self.entries.iter().enumerate().skip(offset).take(visible_height) {
            let is_selected = focused && i == self.selected;

            let bg = if is_selected {
                theme::SELECTED_BG
            } else {
                ratatui::style::Color::Reset
            };

            let label = format!("  stash@{{{}}}: {}", entry.index, entry.message);
            let pad = (inner.width as usize).saturating_sub(label.len());

            lines.push(Line::from(Span::styled(
                format!("{}{}", label, " ".repeat(pad)),
                Style::default()
                    .fg(ratatui::style::Color::White)
                    .bg(bg),
            )));
        }

        Paragraph::new(lines).render(inner, buf);
    }

    fn render_file_list(&self, area: Rect, buf: &mut Buffer) {
        let focused = self.focus == StashViewFocus::FileList;
        let border_color = if focused {
            theme::FOCUS_BORDER
        } else {
            theme::UNFOCUS_BORDER
        };

        let title_style = if focused {
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(border_color)
        };

        let stash_label = if !self.entries.is_empty() {
            format!("stash@{{{}}}", self.entries.get(self.selected).map_or(0, |e| e.index))
        } else {
            String::new()
        };

        let block = Block::default()
            .title(Span::styled(
                format!(" Files in {} ({}) ", stash_label, self.files.len()),
                title_style,
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.files.is_empty() {
            let msg = if self.entries.is_empty() {
                "(no stash selected)"
            } else {
                "(no files)"
            };
            let empty = Paragraph::new(Span::styled(
                msg,
                Style::default().fg(ratatui::style::Color::DarkGray),
            ));
            empty.render(inner, buf);
            return;
        }

        let visible_height = inner.height as usize;
        let offset = if self.file_selected >= visible_height {
            self.file_selected - visible_height + 1
        } else {
            0
        };

        let mut lines: Vec<Line<'static>> = Vec::new();
        for (i, file) in self.files.iter().enumerate().skip(offset).take(visible_height) {
            let is_selected = focused && i == self.file_selected;

            let status_color = match file.status_code {
                FileStatusCode::Modified => theme::STATUS_MODIFIED,
                FileStatusCode::Added => theme::STATUS_ADDED,
                FileStatusCode::Deleted => theme::STATUS_DELETED,
                FileStatusCode::Renamed => theme::STATUS_RENAMED,
                FileStatusCode::Typechange => theme::STATUS_MODIFIED,
                FileStatusCode::Untracked => theme::STATUS_UNTRACKED,
            };

            let bg = if is_selected {
                theme::SELECTED_BG
            } else {
                ratatui::style::Color::Reset
            };

            let letter = file.status_code.letter();
            let path = &file.path;
            let content_len = 4 + path.len();
            let pad = (inner.width as usize).saturating_sub(content_len);

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", letter),
                    Style::default().fg(status_color).bg(bg),
                ),
                Span::styled(
                    format!("{}{}", path, " ".repeat(pad)),
                    Style::default()
                        .fg(ratatui::style::Color::White)
                        .bg(bg),
                ),
            ]));
        }

        Paragraph::new(lines).render(inner, buf);
    }

    fn render_diff_overlay(&self, area: Rect, buf: &mut Buffer) {
        let diff = match self.diff {
            Some(d) => d,
            None => return,
        };

        let block = Block::default()
            .title(Span::styled(
                format!(" Diff: {} ", self.diff_path),
                Style::default()
                    .fg(theme::FOCUS_BORDER)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::FOCUS_BORDER));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines: Vec<Line<'static>> = Vec::new();

        for file in &diff.files {
            for hunk in &file.hunks {
                lines.push(Line::from(Span::styled(
                    hunk.header.clone(),
                    Style::default().fg(theme::DIFF_HUNK),
                )));
                for diff_line in &hunk.lines {
                    let (prefix, color) = match diff_line.origin {
                        '+' => ("+", theme::DIFF_ADD),
                        '-' => ("-", theme::DIFF_DEL),
                        _ => (" ", ratatui::style::Color::Reset),
                    };
                    let content = diff_line.content.trim_end_matches('\n');
                    lines.push(Line::from(Span::styled(
                        format!("{}{}", prefix, content),
                        Style::default().fg(color),
                    )));
                }
                lines.push(Line::from(""));
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no changes)",
                Style::default().fg(ratatui::style::Color::DarkGray),
            )));
        }

        Paragraph::new(lines)
            .scroll((self.diff_scroll, 0))
            .render(inner, buf);
    }
}

impl Widget for StashView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.focus == StashViewFocus::DiffPreview {
            self.render_diff_overlay(area, buf);
            return;
        }

        // Show error at top if present
        let (error_area, main_area) = if self.error.is_some() {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(area);
            (Some(chunks[0]), chunks[1])
        } else {
            (None, area)
        };

        if let (Some(err_area), Some(err)) = (error_area, self.error) {
            Paragraph::new(Span::styled(
                err.to_string(),
                Style::default().fg(ratatui::style::Color::Red),
            ))
            .render(err_area, buf);
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(main_area);

        self.render_stash_list(chunks[0], buf);
        self.render_file_list(chunks[1], buf);
    }
}
