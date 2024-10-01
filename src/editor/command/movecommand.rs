use crossterm::event::{
    KeyCode::{self},
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
    WordForward,
    WordBackward,
    BigWordForward,
    BigWordBackward,
}
impl TryFrom<KeyEvent> for Move {
    type Error = String;
    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        match (event.code, event.modifiers) {
            (KeyCode::Char('h'), KeyModifiers::NONE) => Ok(Self::Left),
            (KeyCode::Char('j'), KeyModifiers::NONE) => Ok(Self::Down),
            (KeyCode::Char('k'), KeyModifiers::NONE) => Ok(Self::Up),
            (KeyCode::Char('l'), KeyModifiers::NONE) => Ok(Self::Right),
            (KeyCode::Char('w'), KeyModifiers::NONE) => Ok(Self::WordForward),
            (KeyCode::Char('b'), KeyModifiers::NONE) => Ok(Self::WordBackward),
            (KeyCode::Char('W'), KeyModifiers::SHIFT) => Ok(Self::BigWordForward),
            (KeyCode::Char('B'), KeyModifiers::SHIFT) => Ok(Self::BigWordBackward),
            _ => Err(format!("Not a move command: {:?}", event))
        }
    }
}
