#[derive(Clone, Copy)]
pub enum Mode {
    Normal,
    Insert,
}

#[derive(Clone)]
pub enum Command {
    Quit,
    None,
    Print(String), // Just for now
    MoveCursorLeft,
    MoveCursorDown,
    MoveCursorUp,
    MoveCursorRight,
    SwitchMode(Mode),
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub x: usize, // Horizontal (row)
    pub y: usize, // Vertical (column)
}

impl Position {
    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

#[derive(Clone, Copy)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}
