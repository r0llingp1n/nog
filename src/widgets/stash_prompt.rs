use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use super::popup::centered_rect;

pub struct StashPrompt<'a> {
    pub input: &'a str,
    pub target_branch: &'a str,
    pub error: Option<&'a str>,
}

impl<'a> Widget for StashPrompt<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        if let Some(err) = self.error {
            lines.push(Line::from(Span::styled(
                err.to_string(),
                Style::default().fg(ratatui::style::Color::Red),
            )));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled(
            format!(
                "Uncommitted changes conflict with switching to '{}'.",
                self.target_branch
            ),
            Style::default().fg(ratatui::style::Color::Yellow),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Stash name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}_", self.input),
                Style::default().fg(ratatui::style::Color::White),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Enter:stash & switch  Esc:cancel",
            Style::default().fg(ratatui::style::Color::DarkGray),
        )));

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
            .title(" Stash Changes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ratatui::style::Color::Yellow));

        let inner = block.inner(popup);
        block.render(popup, buf);

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }
}
