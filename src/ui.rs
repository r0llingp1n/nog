use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use crate::app::{App, AppMode};
use crate::git::diff::DiffData;
use crate::widgets::branch_picker::BranchPicker;
use crate::widgets::commit_detail::CommitDetail;
use crate::widgets::commit_view::CommitView;
use crate::widgets::graph_view::GraphView;
use crate::widgets::stash_prompt::StashPrompt;
use crate::widgets::stash_view::StashView;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    if app.commits.is_empty() {
        let msg = Paragraph::new("No commits found in this repository.")
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    // Reserve 1 row for status bar (or search bar in Search mode)
    let main_area;
    let bottom_area;
    if area.height > 2 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        main_area = chunks[0];
        bottom_area = chunks[1];
    } else {
        main_area = area;
        bottom_area = Rect::default();
    }

    match app.mode {
        AppMode::GraphView | AppMode::BranchPicker | AppMode::StashPrompt | AppMode::Search => {
            let graph = GraphView {
                commits: &app.commits,
                graph_rows: &app.graph_rows,
                decorations: &app.decorations,
                show_remotes: app.show_remotes,
            };
            frame.render_stateful_widget(graph, main_area, &mut app.graph_state);

            if app.mode == AppMode::BranchPicker {
                let picker = BranchPicker {
                    branches: &app.branches,
                    selected: app.branch_selected,
                    current_branch: &app.branch_name,
                    error: app.branch_error.as_deref(),
                };
                frame.render_widget(picker, main_area);
            } else if app.mode == AppMode::StashPrompt {
                let target = app.stash_target_branch.as_deref().unwrap_or("unknown");
                let prompt = StashPrompt {
                    input: &app.stash_input,
                    target_branch: target,
                    error: app.stash_error.as_deref(),
                };
                frame.render_widget(prompt, main_area);
            }
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
                show_remotes: app.show_remotes,
            };
            frame.render_stateful_widget(graph, chunks[0], &mut app.graph_state);

            if let Some(commit) = app.selected_commit().cloned() {
                let empty_diff = DiffData { files: Vec::new() };
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
        AppMode::CommitView => {
            let commit_view = CommitView {
                unstaged: &app.commit_unstaged,
                staged: &app.commit_staged,
                message: &app.commit_message,
                focus: app.commit_focus,
                unstaged_selected: app.commit_unstaged_selected,
                staged_selected: app.commit_staged_selected,
                error: app.commit_error.as_deref(),
                diff: app.commit_diff.as_ref(),
                diff_scroll: app.commit_diff_scroll,
                diff_path: &app.commit_diff_path,
            };
            frame.render_widget(commit_view, main_area);
        }
        AppMode::StashView => {
            let stash_view = StashView {
                entries: &app.stash_entries,
                selected: app.stash_selected,
                focus: app.stash_view_focus,
                files: &app.stash_files,
                file_selected: app.stash_file_selected,
                diff: app.stash_diff.as_ref(),
                diff_scroll: app.stash_diff_scroll,
                diff_path: &app.stash_diff_path,
                error: app.stash_view_error.as_deref(),
            };
            frame.render_widget(stash_view, main_area);
        }
    }

    // Bottom bar
    if bottom_area.height > 0 {
        if app.mode == AppMode::Search {
            render_search_bar(frame, app, bottom_area);
        } else {
            render_status_bar(frame, app, bottom_area);
        }
    }
}

fn render_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    use ratatui::style::Color;

    let bar_bg = Color::Rgb(40, 40, 40);
    let base = Style::default().bg(bar_bg);

    let prompt_char = if app.search_direction == crate::app::SearchDirection::Forward {
        "/"
    } else {
        "?"
    };

    let match_info = if app.search_input.is_empty() {
        String::new()
    } else if app.search_matches.is_empty() {
        " [no matches]".to_string()
    } else {
        let idx = app.search_match_idx.map(|i| i + 1).unwrap_or(0);
        format!(" [{}/{}]", idx, app.search_matches.len())
    };

    let spans = vec![
        Span::styled(
            prompt_char.to_string(),
            base.fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(app.search_input.clone(), base.fg(Color::White)),
        Span::styled("_".to_string(), base.fg(Color::DarkGray)),
        Span::styled(match_info, base.fg(Color::DarkGray)),
    ];

    // Right-aligned help hint
    let help = if app.search_direction == crate::app::SearchDirection::Forward {
        " ?:backsearch  Enter:confirm  Esc:cancel"
    } else {
        " /:search  Enter:confirm  Esc:cancel"
    };

    let used: usize = spans.iter().map(|s| s.content.len()).sum();
    let help_len = help.len();
    let pad = (area.width as usize).saturating_sub(used + help_len);
    let mut all_spans = spans;
    all_spans.push(Span::styled(" ".repeat(pad), base));
    all_spans.push(Span::styled(help.to_string(), base.fg(Color::DarkGray)));

    Paragraph::new(Line::from(all_spans)).render(area, frame.buffer_mut());
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    use ratatui::style::Color;

    let bar_bg = Color::Rgb(40, 40, 40);
    let key_fg = Color::Rgb(200, 200, 200);
    let desc_fg = Color::Rgb(120, 120, 120);
    let branch_fg = Color::Rgb(130, 210, 130);
    let count_fg = Color::Rgb(160, 160, 160);

    let branch = app.current_branch_name();
    let commit_count = app.commits.len();
    let selected = app.graph_state.selected + 1;

    let base = Style::default().bg(bar_bg);

    let mut spans: Vec<Span<'static>> = Vec::new();

    // Branch name
    spans.push(Span::styled(
        format!(" {} ", branch),
        base.fg(branch_fg).add_modifier(Modifier::BOLD),
    ));

    // Remotes indicator
    if app.show_remotes {
        spans.push(Span::styled(" remotes", base.fg(Color::Rgb(210, 130, 130))));
    }

    // Branch filter indicator
    if app.branch_filter {
        spans.push(Span::styled(" branch", base.fg(Color::Rgb(130, 190, 210))));
    }

    // Active search indicator
    if app.search_pattern.is_some() && !app.search_matches.is_empty() {
        let idx = app.search_match_idx.map(|i| i + 1).unwrap_or(0);
        let total = app.search_matches.len();
        spans.push(Span::styled(
            format!(
                " /{} [{}/{}]",
                app.search_pattern.as_deref().unwrap_or(""),
                idx,
                total
            ),
            base.fg(Color::Rgb(210, 210, 130)),
        ));
    }

    spans.push(Span::styled("  ", base));

    // Key hints
    let hints: &[(&str, &str)] = match app.mode {
        AppMode::GraphView | AppMode::BranchPicker | AppMode::StashPrompt | AppMode::Search => &[
            ("c", "commit"),
            ("s", "stash"),
            ("/", "search"),
            ("b", "branch"),
            ("B", "filter"),
            ("r", "remotes"),
            ("q", "quit"),
        ],
        AppMode::QuickView => &[("Space", "close"), ("Esc", "close"), ("j/k", "scroll")],
        AppMode::CommitView => &[
            ("Tab", "focus"),
            ("s/u", "stage/unstage"),
            ("r", "restore"),
            ("c", "commit"),
            ("Esc", "back"),
        ],
        AppMode::StashView => &[
            ("a", "apply"),
            ("p", "pop"),
            ("d", "drop"),
            ("Space", "files/diff"),
            ("Esc", "back"),
        ],
    };

    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ", base));
        }
        spans.push(Span::styled(key.to_string(), base.fg(key_fg)));
        spans.push(Span::styled(format!(":{}", desc), base.fg(desc_fg)));
    }

    // Right side: repo path + version + commit position
    let right = format!(
        " {}  v{}  {}/{} ",
        app.repo_path.to_string_lossy(),
        env!("CARGO_PKG_VERSION"),
        selected,
        commit_count
    );
    let used: usize = spans.iter().map(|s| s.content.len()).sum();
    let padding = (area.width as usize).saturating_sub(used + right.len());
    spans.push(Span::styled(" ".repeat(padding), base));
    spans.push(Span::styled(right, base.fg(count_fg)));

    Paragraph::new(Line::from(spans)).render(area, frame.buffer_mut());
}
