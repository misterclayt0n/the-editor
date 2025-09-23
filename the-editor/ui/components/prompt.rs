use std::collections::VecDeque;

use the_editor_renderer::{Key, KeyPress, Renderer, Color, TextSection};

use crate::{
    core::{
        graphics::Rect,
        command_registry::CommandRegistry,
        commands::Context,
    },
};

/// Events that can occur in the prompt component
#[derive(Debug, Clone, PartialEq)]
pub enum PromptEvent {
    /// Validate and execute the current input
    Validate,
    /// Update the input (continue editing)
    Update,
    /// Abort/cancel the prompt
    Abort,
}

/// A prompt component for handling command input
#[derive(Debug)]
pub struct Prompt {
    /// Current input text
    input: String,
    /// Cursor position within the input
    cursor: usize,
    /// Command history (previous commands)
    history: VecDeque<String>,
    /// Current position in history during navigation
    history_pos: Option<usize>,
    /// Current completions based on input
    completions: Vec<String>,
    /// Index of selected completion
    completion_index: Option<usize>,
    /// Whether completion menu is visible
    show_completions: bool,
    /// Maximum history size
    max_history: usize,
    /// Prefix for the prompt (e.g., ":")
    prefix: String,
}

impl Prompt {
    /// Create a new prompt with the given prefix
    pub fn new(prefix: String) -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            history: VecDeque::new(),
            history_pos: None,
            completions: Vec::new(),
            completion_index: None,
            show_completions: false,
            max_history: 100,
            prefix,
        }
    }

    /// Handle a key press and return the appropriate event
    pub fn handle_key(&mut self, key: KeyPress, registry: &CommandRegistry) -> PromptEvent {
        if !key.pressed {
            return PromptEvent::Update;
        }

        match key.code {
            Key::Enter => {
                // Execute command
                if !self.input.trim().is_empty() {
                    self.add_to_history(self.input.clone());
                }
                PromptEvent::Validate
            }
            Key::Escape => {
                // Cancel prompt
                PromptEvent::Abort
            }
            Key::Backspace => {
                self.delete_char_backward();
                self.update_completions(registry);
                PromptEvent::Update
            }
            Key::Delete => {
                self.delete_char_forward();
                self.update_completions(registry);
                PromptEvent::Update
            }
            Key::Left => {
                self.move_cursor_left();
                PromptEvent::Update
            }
            Key::Right => {
                self.move_cursor_right();
                PromptEvent::Update
            }
            Key::Up => {
                if self.show_completions {
                    self.completion_previous();
                } else {
                    self.history_previous();
                }
                PromptEvent::Update
            }
            Key::Down => {
                if self.show_completions {
                    self.completion_next();
                } else {
                    self.history_next();
                }
                PromptEvent::Update
            }
            Key::Tab => {
                if self.show_completions {
                    self.complete_current();
                } else {
                    self.update_completions(registry);
                    self.show_completions = !self.completions.is_empty();
                }
                PromptEvent::Update
            }
            Key::Home => {
                self.cursor = 0;
                PromptEvent::Update
            }
            Key::End => {
                self.cursor = self.input.len();
                PromptEvent::Update
            }
            Key::Char(c) => {
                self.insert_char(c);
                self.update_completions(registry);
                PromptEvent::Update
            }
            _ => PromptEvent::Update,
        }
    }

    /// Get the current input text
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Set the input text
    pub fn set_input(&mut self, input: String) {
        self.input = input;
        self.cursor = self.input.len();
        self.show_completions = false;
        self.completion_index = None;
    }

    /// Clear the prompt
    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.show_completions = false;
        self.completion_index = None;
        self.history_pos = None;
    }

    /// Render the prompt to the screen
    pub fn render(&self, renderer: &mut Renderer, area: Rect) {
        let prompt_text = format!("{}{}", self.prefix, self.input);

        // Render prompt text
        renderer.draw_text(TextSection::simple(
            area.x as f32,
            area.y as f32,
            &prompt_text,
            14.0,
            Color::WHITE,
        ));

        // Render cursor - simple cursor rendering for now
        let cursor_col = self.prefix.len() + self.cursor;
        let cursor_x = area.x as f32 + cursor_col as f32 * 8.0; // Assuming font width of 8

        // Draw cursor as a simple vertical bar
        // Note: This is a simplified cursor rendering - in a real implementation
        // you'd use the proper cursor rendering API
        if cursor_col <= prompt_text.len() {
            renderer.draw_text(TextSection::simple(
                cursor_x,
                area.y as f32,
                "|",
                14.0,
                Color::rgb(0.8, 0.8, 0.8),
            ));
        }

        // Render completions if visible
        if self.show_completions && !self.completions.is_empty() {
            self.render_completions(renderer, area);
        }
    }

    /// Execute the current command
    pub fn execute(&self, cx: &mut Context) -> anyhow::Result<()> {
        let input = self.input.trim();
        if input.is_empty() {
            return Ok(());
        }

        // Parse command and arguments
        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts[0];
        let args = &parts[1..];

        // Clone the command registry to avoid borrowing issues
        let registry = cx.editor.command_registry.clone();

        // Execute through command registry
        registry.execute(cx, command, args)
    }

    // Private methods

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
        self.show_completions = false;
        self.completion_index = None;
    }

    fn delete_char_backward(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
            self.show_completions = false;
            self.completion_index = None;
        }
    }

    fn delete_char_forward(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
            self.show_completions = false;
            self.completion_index = None;
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    fn update_completions(&mut self, registry: &CommandRegistry) {
        let input = self.input.trim();
        if input.is_empty() {
            self.completions.clear();
            self.show_completions = false;
            self.completion_index = None;
            return;
        }

        // Get command name (first word)
        let command_prefix = input.split_whitespace().next().unwrap_or("");

        if !command_prefix.is_empty() {
            self.completions = registry
                .completions(command_prefix)
                .into_iter()
                .map(|s| s.to_string())
                .collect();
        } else {
            self.completions.clear();
        }

        self.completion_index = if self.completions.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    fn completion_next(&mut self) {
        if let Some(index) = self.completion_index {
            if !self.completions.is_empty() {
                self.completion_index = Some((index + 1) % self.completions.len());
            }
        }
    }

    fn completion_previous(&mut self) {
        if let Some(index) = self.completion_index {
            if !self.completions.is_empty() {
                self.completion_index = Some(
                    if index == 0 {
                        self.completions.len() - 1
                    } else {
                        index - 1
                    }
                );
            }
        }
    }

    fn complete_current(&mut self) {
        if let Some(index) = self.completion_index {
            if let Some(completion) = self.completions.get(index) {
                // Replace the current command with the completion
                let parts: Vec<&str> = self.input.split_whitespace().collect();
                if !parts.is_empty() {
                    let mut new_input = completion.clone();
                    if parts.len() > 1 {
                        new_input.push(' ');
                        new_input.push_str(&parts[1..].join(" "));
                    }
                    self.input = new_input;
                    self.cursor = self.input.len();
                    self.show_completions = false;
                    self.completion_index = None;
                }
            }
        }
    }

    fn add_to_history(&mut self, command: String) {
        // Don't add duplicate consecutive entries
        if self.history.back() == Some(&command) {
            return;
        }

        self.history.push_back(command);

        // Limit history size
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }

        self.history_pos = None;
    }

    fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_pos = match self.history_pos {
            None => self.history.len() - 1,
            Some(pos) => {
                if pos > 0 {
                    pos - 1
                } else {
                    return; // Already at oldest
                }
            }
        };

        self.history_pos = Some(new_pos);
        if let Some(cmd) = self.history.get(new_pos) {
            self.input = cmd.clone();
            self.cursor = self.input.len();
        }
    }

    fn history_next(&mut self) {
        match self.history_pos {
            None => return, // Not in history mode
            Some(pos) => {
                if pos + 1 < self.history.len() {
                    let new_pos = pos + 1;
                    self.history_pos = Some(new_pos);
                    if let Some(cmd) = self.history.get(new_pos) {
                        self.input = cmd.clone();
                        self.cursor = self.input.len();
                    }
                } else {
                    // Return to current input
                    self.history_pos = None;
                    self.input.clear();
                    self.cursor = 0;
                }
            }
        }
    }

    fn render_completions(&self, renderer: &mut Renderer, area: Rect) {
        if self.completions.is_empty() {
            return;
        }

        let completion_area = Rect {
            x: area.x,
            y: area.y + 25, // Below the prompt
            width: 300,
            height: (self.completions.len() as u16 * 20).min(200),
        };

        // Draw completions (simplified - no background for now)
        for (i, completion) in self.completions.iter().take(10).enumerate() {
            let y = completion_area.y as f32 + i as f32 * 20.0;
            let is_selected = self.completion_index == Some(i);

            let color = if is_selected {
                Color::rgb(0.8, 0.8, 1.0) // Lighter blue for selected
            } else {
                Color::rgb(0.7, 0.7, 0.7) // Gray for normal
            };

            renderer.draw_text(TextSection::simple(
                completion_area.x as f32 + 5.0,
                y + 2.0,
                completion,
                12.0,
                color,
            ));
        }
    }
}

impl Default for Prompt {
    fn default() -> Self {
        Self::new(":".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::command_registry::CommandRegistry;

    #[test]
    fn test_prompt_creation() {
        let prompt = Prompt::new(":".to_string());
        assert_eq!(prompt.input(), "");
        assert_eq!(prompt.cursor, 0);
    }

    #[test]
    fn test_prompt_input() {
        let mut prompt = Prompt::new(":".to_string());
        let registry = CommandRegistry::new();

        prompt.insert_char('q');
        assert_eq!(prompt.input(), "q");
        assert_eq!(prompt.cursor, 1);

        prompt.insert_char('u');
        prompt.insert_char('i');
        prompt.insert_char('t');
        assert_eq!(prompt.input(), "quit");
        assert_eq!(prompt.cursor, 4);
    }

    #[test]
    fn test_prompt_backspace() {
        let mut prompt = Prompt::new(":".to_string());
        prompt.set_input("quit".to_string());

        prompt.delete_char_backward();
        assert_eq!(prompt.input(), "qui");
        assert_eq!(prompt.cursor, 3);
    }

    #[test]
    fn test_prompt_completions() {
        let mut prompt = Prompt::new(":".to_string());
        let registry = CommandRegistry::new();

        prompt.set_input("q".to_string());
        prompt.update_completions(&registry);

        assert!(!prompt.completions.is_empty());
        assert!(prompt.completions.contains(&"quit".to_string()));
    }
}