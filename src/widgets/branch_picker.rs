use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use super::popup::centered_rect;

pub struct BranchPicker<'a> {
    pub branches: &'a [String],
    pub selected: usize,
    pub current_branch: &'a str,
    pub error: Option<&'a str>,
}

impl<'a> Widget for BranchPicker<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Error message if any
        if let Some(err) = self.error {
            lines.push(Line::from(Span::styled(
                err.to_string(),
                Style::default().fg(ratatui::style::Color::Red),
            )));
            lines.push(Line::from(""));
        }

        for (i, branch) in self.branches.iter().enumerate() {
            let is_selected = i == self.selected;
            let is_current = branch == self.current_branch;

            let marker = if is_current { "* " } else { "  " };
            let label = format!("{}{}", marker, branch);

            let style = if is_selected {
                Style::default()
                    .bg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default().fg(ratatui::style::Color::Green)
            } else {
                Style::default()
            };

            lines.push(Line::from(Span::styled(label, style)));
        }

        // Hint at the bottom
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Enter:checkout  Esc:cancel".to_string(),
            Style::default().fg(ratatui::style::Color::DarkGray),
        )));

        // Size popup to content: height = lines + 2 (border), width = widest line + 4 (border + pad)
        let content_width = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.len()).sum::<usize>())
            .max()
            .unwrap_or(20);
        let popup_width = (content_width as u16 + 4).max(24).min(area.width.saturating_sub(4));
        let popup_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
        let popup = centered_rect(popup_width, popup_height, area);

        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Switch Branch ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ratatui::style::Color::Cyan));

        let inner = block.inner(popup);
        block.render(popup, buf);

        Paragraph::new(lines)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }
}
