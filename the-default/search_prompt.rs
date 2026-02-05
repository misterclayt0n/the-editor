use crate::Direction;

#[derive(Debug, Clone)]
pub struct SearchPromptState {
  pub active:      bool,
  pub direction:   Direction,
  pub query:       String,
  pub cursor:      usize,
  pub completions: Vec<String>,
  pub error:       Option<String>,
  pub register:    char,
  pub extend:      bool,
}

impl SearchPromptState {
  pub fn new() -> Self {
    Self {
      active: false,
      direction: Direction::Forward,
      query: String::new(),
      cursor: 0,
      completions: Vec::new(),
      error: None,
      register: '/',
      extend: false,
    }
  }

  pub fn clear(&mut self) {
    self.active = false;
    self.query.clear();
    self.cursor = 0;
    self.completions.clear();
    self.error = None;
    self.register = '/';
    self.extend = false;
  }
}

impl Default for SearchPromptState {
  fn default() -> Self {
    Self::new()
  }
}
