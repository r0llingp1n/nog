use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::app::ConflictFocus;
use crate::theme;

/// View shown after a sync left a merge conflict. Lists the conflicted files
/// and, for the selected one, its working-tree content (with conflict markers)
/// so the user can resolve it externally, then re-check (`r`) and finalize (`c`)
/// or abort (`Esc`) to restore the pre-merge state.
pub struct ConflictView<'a> {
    pub files: &'a [String],
    pub selected: usize,
    pub focus: ConflictFocus,
    pub content: Option<&'a str>,
    pub scroll: u16,
    pub error: Option<&'a str>,
}

impl ConflictView<'_> {
    fn border(focused: bool) -> (ratatui::style::Color, Style) {
        let color = if focused {
            theme::FOCUS_BORDER
        } else {
            theme::UNFOCUS_BORDER
        };
        let style = if focused {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };
        (color, style)
    }

    fn render_file_list(&self, area: Rect, buf: &mut Buffer) {
        let focused = self.focus == ConflictFocus::Files;
        let (border_color, title_style) = Self::border(focused);

        let block = Block::default()
            .title(Span::styled(
                format!(" Merge conflict ({} file(s)) ", self.files.len()),
                title_style,
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.files.is_empty() {
            let empty = Paragraph::new(Span::styled(
                "(no conflicts)",
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
        for (i, path) in self
            .files
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_height)
        {
            let is_selected = focused && i == self.selected;
            let bg = if is_selected {
                theme::SELECTED_BG
            } else {
                ratatui::style::Color::Reset
            };
            let content_len = 4 + path.len();
            let pad = (inner.width as usize).saturating_sub(content_len);

            lines.push(Line::from(vec![
                Span::styled(
                    "  U ".to_string(),
                    Style::default().fg(theme::STATUS_CONFLICTED).bg(bg),
                ),
                Span::styled(
                    format!("{}{}", path, " ".repeat(pad)),
                    Style::default().fg(ratatui::style::Color::White).bg(bg),
                ),
            ]));
        }

        Paragraph::new(lines).render(inner, buf);
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        let focused = self.focus == ConflictFocus::Content;
        let (border_color, title_style) = Self::border(focused);

        let block = Block::default()
            .title(Span::styled(
                " Working tree: resolve, then 'r' to re-check or 'c' to commit ".to_string(),
                title_style,
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        let content = match self.content {
            Some(c) => c,
            None => {
                let hint = Paragraph::new(Span::styled(
                    "(select a file and press Space to view its working-tree content)",
                    Style::default().fg(ratatui::style::Color::DarkGray),
                ));
                hint.render(inner, buf);
                return;
            }
        };

        let lines: Vec<Line<'static>> = content
            .lines()
            .map(|l| {
                let color = if l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>") {
                    theme::STATUS_CONFLICTED
                } else if l.starts_with("=======") {
                    theme::DIFF_META
                } else {
                    ratatui::style::Color::Reset
                };
                Line::from(vec![Span::styled(
                    l.to_string(),
                    Style::default().fg(color),
                )])
            })
            .collect();

        Paragraph::new(lines)
            .scroll((self.scroll, 0))
            .render(inner, buf);
    }
}

impl Widget for ConflictView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Error banner at top if present.
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

        self.render_file_list(chunks[0], buf);
        self.render_content(chunks[1], buf);
    }
}
