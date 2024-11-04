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
    SwitchMode(Mode)
}

#[derive(Clone, Copy)]
pub struct Position {
    pub x: usize,
    pub y: usize
}

impl Position {
    pub fn zero() -> Self {
        Self {
            x: 0,
            y: 0,
        }
    }
}
