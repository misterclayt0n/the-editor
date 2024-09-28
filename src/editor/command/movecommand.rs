use crossterm::event::{
    KeyCode::{self, Down, End, Home, Left, PageDown, PageUp, Right, Up},
    KeyEvent, KeyModifiers,
};

#[derive(Clone, Copy)]
pub enum Move {
    PageUp,
    PageDown,
    StartOfLine,
    EndOfLine,
    Up,
    Left,
    Right,
    Down,
}
impl TryFrom<KeyEvent> for Move {
    type Error = String;
    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        match (event.code, event.modifiers) {
            (KeyCode::Char('h'), KeyModifiers::NONE) => Ok(Self::Left),
            (KeyCode::Char('j'), KeyModifiers::NONE) => Ok(Self::Down),
            (KeyCode::Char('k'), KeyModifiers::NONE) => Ok(Self::Up),
            (KeyCode::Char('l'), KeyModifiers::NONE) => Ok(Self::Right),
            _ => Err(format!("Not a move command: {:?}", event))
        }
    }
}
