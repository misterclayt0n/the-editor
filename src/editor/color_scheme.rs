use crossterm::style::Color;

pub struct ColorScheme {
    pub background: Color,
    pub foreground: Color,
    pub selection_background: Color,
    pub selection_foreground: Color,
    pub search_match_background: Color,
    pub search_match_foreground: Color,
}

// define basic default colors
impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::Reset,
            selection_background: Color::DarkGrey,
            selection_foreground: Color::White,
            search_match_background: Color::Yellow,
            search_match_foreground: Color::Black,
        }
    }
}
