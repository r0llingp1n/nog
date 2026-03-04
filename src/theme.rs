use ratatui::style::Color;

pub const GRAPH_COLORS: [Color; 8] = [
    Color::Green,
    Color::Red,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::LightGreen,
    Color::LightRed,
];

pub const HASH_COLOR: Color = Color::Yellow;
pub const DATE_COLOR: Color = Color::DarkGray;
pub const AUTHOR_COLOR: Color = Color::Blue;
pub const SELECTED_BG: Color = Color::DarkGray;

pub const DIFF_ADD: Color = Color::Green;
pub const DIFF_DEL: Color = Color::Red;
pub const DIFF_HUNK: Color = Color::Cyan;
pub const DIFF_META: Color = Color::Yellow;

pub const STATUS_MODIFIED: Color = Color::Yellow;
pub const STATUS_ADDED: Color = Color::Green;
pub const STATUS_DELETED: Color = Color::Red;
pub const STATUS_UNTRACKED: Color = Color::Cyan;
pub const STATUS_RENAMED: Color = Color::Magenta;
pub const FOCUS_BORDER: Color = Color::Cyan;
pub const UNFOCUS_BORDER: Color = Color::DarkGray;

pub fn graph_color(index: usize) -> Color {
    GRAPH_COLORS[index % GRAPH_COLORS.len()]
}
