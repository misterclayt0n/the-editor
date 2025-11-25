//! Context building and visualization for ACP prompts.
//!
//! This module provides tools for building the context that gets sent to the
//! ACP agent, as well as utilities for visualizing what context is being sent.

use std::path::PathBuf;

use ropey::RopeSlice;

use crate::core::{
  document::Document,
  selection::Range,
  view::View,
};

/// Context information gathered for an ACP prompt.
#[derive(Debug, Clone)]
pub struct PromptContext {
  /// The selected text that forms the prompt
  pub selection_text: String,
  /// The file path of the current document (if any)
  pub file_path: Option<PathBuf>,
  /// The language of the current document (if detected)
  pub language: Option<String>,
  /// Line number where the selection starts (1-indexed)
  pub start_line: usize,
  /// Line number where the selection ends (1-indexed)
  pub end_line: usize,
  /// Lines before the selection for context
  pub lines_before: String,
  /// Lines after the selection for context
  pub lines_after: String,
  /// The workspace root directory
  pub workspace_root: Option<PathBuf>,
}

impl PromptContext {
  /// Build context from a document selection.
  pub fn from_selection(
    doc: &Document,
    _view: &View,
    selection: &Range,
    context_lines: usize,
  ) -> Self {
    let text = doc.text().slice(..);

    // Get the selected text
    let selection_text = selection.fragment(text).to_string();

    // Get line numbers
    let start_char = selection.from();
    let end_char = selection.to();
    let start_line = text.char_to_line(start_char) + 1; // 1-indexed
    let end_line = text.char_to_line(end_char.saturating_sub(1).max(start_char)) + 1;

    // Get surrounding context
    let (lines_before, lines_after) =
      Self::extract_surrounding_lines(text, start_line - 1, end_line - 1, context_lines);

    // Get language
    let language = doc
      .language_config()
      .map(|lc| lc.language_id.clone())
      .or_else(|| {
        doc
          .path()
          .and_then(|p| p.extension())
          .and_then(|e| e.to_str())
          .map(|s| s.to_string())
      });

    Self {
      selection_text,
      file_path: doc.path().map(|p| p.to_path_buf()),
      language,
      start_line,
      end_line,
      lines_before,
      lines_after,
      workspace_root: None, // Set by caller if needed
    }
  }

  /// Extract lines before and after a selection range.
  fn extract_surrounding_lines(
    text: RopeSlice,
    start_line: usize, // 0-indexed
    end_line: usize,   // 0-indexed
    context_lines: usize,
  ) -> (String, String) {
    let total_lines = text.len_lines();

    // Lines before
    let before_start = start_line.saturating_sub(context_lines);
    let mut lines_before = String::new();
    for line_idx in before_start..start_line {
      if line_idx < total_lines {
        lines_before.push_str(&text.line(line_idx).to_string());
      }
    }

    // Lines after
    let after_start = end_line + 1;
    let after_end = (after_start + context_lines).min(total_lines);
    let mut lines_after = String::new();
    for line_idx in after_start..after_end {
      if line_idx < total_lines {
        lines_after.push_str(&text.line(line_idx).to_string());
      }
    }

    (lines_before, lines_after)
  }

  /// Format the context as a prompt string for the agent.
  ///
  /// This creates a structured prompt that includes file information and context.
  pub fn format_prompt(&self) -> String {
    let mut prompt = String::new();

    // Add file context if available
    if let Some(path) = &self.file_path {
      prompt.push_str(&format!("File: {}\n", path.display()));
    }

    if let Some(lang) = &self.language {
      prompt.push_str(&format!("Language: {}\n", lang));
    }

    prompt.push_str(&format!(
      "Lines: {}-{}\n",
      self.start_line, self.end_line
    ));
    prompt.push('\n');

    // Add the selection as the main prompt
    prompt.push_str(&self.selection_text);

    prompt
  }

  /// Format the context with surrounding code for richer context.
  pub fn format_prompt_with_context(&self) -> String {
    let mut prompt = String::new();

    // Add file context if available
    if let Some(path) = &self.file_path {
      prompt.push_str(&format!("File: {}\n", path.display()));
    }

    if let Some(lang) = &self.language {
      prompt.push_str(&format!("Language: {}\n", lang));
    }

    prompt.push_str(&format!(
      "Selection at lines {}-{}\n\n",
      self.start_line, self.end_line
    ));

    // Add context before
    if !self.lines_before.is_empty() {
      prompt.push_str("--- Context before ---\n");
      prompt.push_str(&self.lines_before);
      if !self.lines_before.ends_with('\n') {
        prompt.push('\n');
      }
    }

    // Add selection (the main prompt)
    prompt.push_str("--- Selected text (your task) ---\n");
    prompt.push_str(&self.selection_text);
    if !self.selection_text.ends_with('\n') {
      prompt.push('\n');
    }

    // Add context after
    if !self.lines_after.is_empty() {
      prompt.push_str("--- Context after ---\n");
      prompt.push_str(&self.lines_after);
    }

    prompt
  }
}

/// Utility for visualizing context that would be sent to the agent.
pub struct ContextVisualizer;

impl ContextVisualizer {
  /// Generate a debug view of the context.
  pub fn format_debug(ctx: &PromptContext) -> String {
    let mut output = String::new();

    output.push_str("=== ACP Context Preview ===\n\n");

    // File info
    output.push_str("FILE INFORMATION\n");
    output.push_str(&"-".repeat(40));
    output.push('\n');

    if let Some(path) = &ctx.file_path {
      output.push_str(&format!("  Path:     {}\n", path.display()));
    } else {
      output.push_str("  Path:     [unsaved buffer]\n");
    }

    if let Some(lang) = &ctx.language {
      output.push_str(&format!("  Language: {}\n", lang));
    }

    output.push_str(&format!(
      "  Lines:    {} to {}\n",
      ctx.start_line, ctx.end_line
    ));

    if let Some(root) = &ctx.workspace_root {
      output.push_str(&format!("  Workspace: {}\n", root.display()));
    }

    output.push('\n');

    // Selection
    output.push_str("SELECTED TEXT (PROMPT)\n");
    output.push_str(&"-".repeat(40));
    output.push('\n');

    let selection_preview = if ctx.selection_text.len() > 500 {
      format!(
        "{}...\n[{} more characters]",
        &ctx.selection_text[..500],
        ctx.selection_text.len() - 500
      )
    } else {
      ctx.selection_text.clone()
    };
    output.push_str(&selection_preview);

    if !selection_preview.ends_with('\n') {
      output.push('\n');
    }

    output.push('\n');

    // Context stats
    output.push_str("SURROUNDING CONTEXT\n");
    output.push_str(&"-".repeat(40));
    output.push('\n');

    let before_lines = ctx.lines_before.lines().count();
    let after_lines = ctx.lines_after.lines().count();
    output.push_str(&format!("  Lines before: {}\n", before_lines));
    output.push_str(&format!("  Lines after:  {}\n", after_lines));

    // Show preview of before context
    if !ctx.lines_before.is_empty() {
      output.push_str("\n  Before preview:\n");
      for (i, line) in ctx.lines_before.lines().take(5).enumerate() {
        let truncated = if line.len() > 60 {
          format!("{}...", &line[..60])
        } else {
          line.to_string()
        };
        output.push_str(&format!("    {:>3}| {}\n", i + 1, truncated));
      }
      if before_lines > 5 {
        output.push_str(&format!("    ... and {} more lines\n", before_lines - 5));
      }
    }

    // Show preview of after context
    if !ctx.lines_after.is_empty() {
      output.push_str("\n  After preview:\n");
      for (i, line) in ctx.lines_after.lines().take(5).enumerate() {
        let truncated = if line.len() > 60 {
          format!("{}...", &line[..60])
        } else {
          line.to_string()
        };
        output.push_str(&format!("    {:>3}| {}\n", i + 1, truncated));
      }
      if after_lines > 5 {
        output.push_str(&format!("    ... and {} more lines\n", after_lines - 5));
      }
    }

    output.push('\n');
    output.push_str(&"=".repeat(42));
    output.push('\n');

    output
  }
}
