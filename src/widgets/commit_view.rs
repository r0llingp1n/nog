use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::app::CommitFocus;
use crate::git::diff::DiffData;
use crate::git::status::{FileStatus, FileStatusCode};
use crate::theme;

pub struct CommitView<'a> {
    pub unstaged: &'a [FileStatus],
    pub staged: &'a [FileStatus],
    pub message: &'a str,
    pub focus: CommitFocus,
    pub unstaged_selected: usize,
    pub staged_selected: usize,
    pub error: Option<&'a str>,
    pub diff: Option<&'a DiffData>,
    pub diff_scroll: u16,
    pub diff_path: &'a str,
}

impl CommitView<'_> {
    fn render_file_list(
        &self,
        files: &[FileStatus],
        selected: usize,
        focused: bool,
        title: &str,
        area: Rect,
        buf: &mut Buffer,
    ) {
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
                format!(" {} ({}) ", title, files.len()),
                title_style,
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if files.is_empty() {
            let empty = Paragraph::new(Span::styled(
                "(no files)",
                Style::default().fg(ratatui::style::Color::DarkGray),
            ));
            empty.render(inner, buf);
            return;
        }

        // Calculate visible range with scrolling
        let visible_height = inner.height as usize;
        let offset = if selected >= visible_height {
            selected - visible_height + 1
        } else {
            0
        };

        let mut lines: Vec<Line<'static>> = Vec::new();
        for (i, file) in files.iter().enumerate().skip(offset).take(visible_height) {
            let is_selected = focused && i == selected;

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

            // Pad to fill the line width
            let content_len = 3 + path.len(); // "  M " + path
            let pad = (inner.width as usize).saturating_sub(content_len);

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", letter),
                    Style::default().fg(status_color).bg(bg),
                ),
                Span::styled(
                    format!("{}{}", path, " ".repeat(pad)),
                    Style::default().fg(ratatui::style::Color::White).bg(bg),
                ),
            ]));
        }

        Paragraph::new(lines).render(inner, buf);
    }

    fn render_message_editor(&self, focused: bool, area: Rect, buf: &mut Buffer) {
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
            .title(Span::styled(" Commit Message ", title_style))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Show error if present
        if let Some(err) = self.error {
            lines.push(Line::from(Span::styled(
                err.to_string(),
                Style::default().fg(ratatui::style::Color::Red),
            )));
            lines.push(Line::from(""));
        }

        // Show message with cursor
        if focused {
            let display = format!("{}_", self.message);
            for line in display.lines() {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(ratatui::style::Color::White),
                )));
            }
            // Handle trailing newline (show cursor on new line)
            if self.message.ends_with('\n') || self.message.is_empty() {
                if self.message.is_empty() {
                    // Already showed "_" above
                } else {
                    // The split won't produce a trailing empty item for ends_with('\n')
                    // but format!("{}_", msg) handles it -- the "_" is on the last segment
                }
            }
        } else if self.message.is_empty() {
            lines.push(Line::from(Span::styled(
                "(enter commit message)",
                Style::default().fg(ratatui::style::Color::DarkGray),
            )));
        } else {
            for line in self.message.lines() {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(ratatui::style::Color::White),
                )));
            }
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

impl Widget for CommitView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.focus == CommitFocus::DiffPreview {
            self.render_diff_overlay(area, buf);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(35),
                Constraint::Percentage(30),
                Constraint::Percentage(35),
            ])
            .split(area);

        self.render_file_list(
            self.unstaged,
            self.unstaged_selected,
            self.focus == CommitFocus::UnstagedFiles,
            "Unstaged Changes",
            chunks[0],
            buf,
        );

        self.render_file_list(
            self.staged,
            self.staged_selected,
            self.focus == CommitFocus::StagedFiles,
            "Staged Changes",
            chunks[1],
            buf,
        );

        self.render_message_editor(self.focus == CommitFocus::MessageEditor, chunks[2], buf);
    }
}
