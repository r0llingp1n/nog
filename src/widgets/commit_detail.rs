use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use crate::git::diff::DiffData;
use crate::git::types::CommitInfo;
use crate::theme;

pub struct CommitDetailState {
    pub scroll: u16,
}

impl CommitDetailState {
    pub fn new() -> Self {
        Self { scroll: 0 }
    }
}

pub struct CommitDetail<'a> {
    pub commit: &'a CommitInfo,
    pub message: &'a str,
    pub diff: &'a DiffData,
    pub scroll: u16,
}

impl<'a> Widget for CommitDetail<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Header
        lines.push(Line::from(vec![
            Span::styled("Commit: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}", self.commit.oid),
                Style::default().fg(theme::HASH_COLOR),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Author: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                self.commit.author_name.clone(),
                Style::default().fg(theme::AUTHOR_COLOR),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Date:   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(self.commit.author_date.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
        lines.push(Line::from(""));

        // Full message
        for line in self.message.lines() {
            lines.push(Line::from(format!("    {}", line)));
        }
        lines.push(Line::from(""));

        // Diff
        for file in &self.diff.files {
            lines.push(Line::from(Span::styled(
                format!("--- {}", file.path),
                Style::default()
                    .fg(theme::DIFF_META)
                    .add_modifier(Modifier::BOLD),
            )));
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
            }
            lines.push(Line::from(""));
        }

        if self.diff.files.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no changes)",
                Style::default().fg(theme::DATE_COLOR),
            )));
        }

        let block = Block::default()
            .borders(Borders::TOP)
            .title(" Quick View ");

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        paragraph.render(area, buf);
    }
}
