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
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    SwitchMode(Mode)
}
