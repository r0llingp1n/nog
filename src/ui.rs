use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use crate::app::{App, AppMode};
use crate::git::diff::DiffData;
use crate::widgets::commit_detail::CommitDetail;
use crate::widgets::graph_view::GraphView;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    if app.commits.is_empty() {
        let msg = Paragraph::new("No commits found in this repository.")
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    // Reserve 1 row for status bar
    let main_area;
    let status_area;
    if area.height > 2 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        main_area = chunks[0];
        status_area = chunks[1];
    } else {
        main_area = area;
        status_area = Rect::default();
    }

    match app.mode {
        AppMode::GraphView => {
            let graph = GraphView {
                commits: &app.commits,
                graph_rows: &app.graph_rows,
                decorations: &app.decorations,
            };
            frame.render_stateful_widget(graph, main_area, &mut app.graph_state);
        }
        AppMode::QuickView => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(main_area);

            let graph = GraphView {
                commits: &app.commits,
                graph_rows: &app.graph_rows,
                decorations: &app.decorations,
            };
            frame.render_stateful_widget(graph, chunks[0], &mut app.graph_state);

            if let Some(commit) = app.selected_commit().cloned() {
                let empty_diff = DiffData {
                    files: Vec::new(),
                };
                let message = app.selected_message.as_deref().unwrap_or("");
                let diff_data = app.selected_diff.as_ref().unwrap_or(&empty_diff);

                let detail = CommitDetail {
                    commit: &commit,
                    message,
                    diff: diff_data,
                    scroll: app.detail_state.scroll,
                };
                frame.render_widget(detail, chunks[1]);
            }
        }
    }

    // Status bar
    if status_area.height > 0 {
        render_status_bar(frame, app, status_area);
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let branch = app.current_branch_name();
    let commit_count = app.commits.len();
    let selected = app.graph_state.selected + 1;

    let left = format!(" {} ", branch);
    let right = format!(" {}/{} commits ", selected, commit_count);
    let mid_hint = match app.mode {
        AppMode::GraphView => " Space:detail  q:quit ",
        AppMode::QuickView => " Space/Esc:close  j/k:scroll ",
    };

    let padding = area
        .width
        .saturating_sub((left.len() + right.len() + mid_hint.len()) as u16);

    let line = Line::from(vec![
        Span::styled(
            left,
            Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(ratatui::style::Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            mid_hint.to_string(),
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .bg(ratatui::style::Color::Black),
        ),
        Span::styled(
            " ".repeat(padding as usize),
            Style::default().bg(ratatui::style::Color::Black),
        ),
        Span::styled(
            right,
            Style::default()
                .fg(ratatui::style::Color::White)
                .bg(ratatui::style::Color::DarkGray),
        ),
    ]);

    Paragraph::new(line).render(area, frame.buffer_mut());
}
