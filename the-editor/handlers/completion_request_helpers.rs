/// Helper functions for completion management
///
/// These functions are used by hooks to manage completion state in a centralized way,
/// following Helix's architecture pattern.

use crate::{
  core::{
    chars::char_is_word,
    commands,
  },
  editor::Editor,
  handlers::completion::CompletionEvent,
  ui,
};

/// Update the completion filter with a new character (or None for backspace)
///
/// This is called when a character is typed while completion is active.
/// It updates the filter, closes if needed, and re-triggers if appropriate.
pub fn update_completion_filter(cx: &mut commands::Context, c: Option<char>) {
  cx.callback.push(Box::new(move |compositor, cx| {
    let Some(editor_view) = compositor.find::<ui::EditorView>() else {
      return;
    };

    if let Some(completion) = &mut editor_view.completion {
      completion.update_filter(c);

      // Close completion if:
      // 1. Filter resulted in no matches, OR
      // 2. Character is not a word character (e.g., space, dot)
      if completion.is_empty() || c.is_some_and(|c| !char_is_word(c)) {
        editor_view.completion = None;
        cx.editor.last_completion = None;

        // After clearing, try to re-trigger auto-completion
        // This handles the case where we typed a trigger char
        if c.is_some() {
          trigger_auto_completion(cx.editor, false);
        }
      } else {
        // Completion still has matches, but if it's incomplete,
        // we should request updated completions from the server

        // Check if any active completions are marked as incomplete
        let has_incomplete = cx.editor
          .handlers
          .completions
          .active_completions
          .lock()
          .unwrap()
          .values()
          .any(|context| context.is_incomplete);

        if has_incomplete {
          // TODO: Call request_incomplete_completion_list when we have TaskController support
          // For now, just log that we would re-request
        }
      }
    }
  }))
}

/// Clear the completion popup and reset state
///
/// This is called when a command is executed that should dismiss completions.
pub fn clear_completions(cx: &mut commands::Context) {
  cx.callback.push(Box::new(|compositor, cx| {
    let Some(editor_view) = compositor.find::<ui::EditorView>() else {
      return;
    };

    editor_view.completion = None;
    cx.editor.last_completion = None;
    // TODO: Cancel in-flight requests via task controller
  }))
}

/// Trigger auto-completion based on current editor state
///
/// This checks if the current context warrants showing completions:
/// - Trigger characters (like `.` in many languages)
/// - Path completion (after `/` or `\`)
/// - Word completion (after typing N word characters)
pub fn trigger_auto_completion(editor: &Editor, trigger_char_only: bool) {
  use the_editor_stdx::rope::RopeSliceExt;

  let (view, doc) = crate::current_ref!(editor);
  let text = doc.text().slice(..);
  let cursor = doc.selection(view.id).primary().cursor(text);

  // Check if this is a trigger character
  let is_trigger_char = doc.language_servers().any(|ls| {
    matches!(&ls.capabilities().completion_provider,
      Some(cap) if cap.trigger_characters.as_ref()
        .map_or(false, |chars| {
          let trigger_text = text.slice(..cursor);
          chars.iter().any(|trigger| trigger_text.ends_with(trigger.as_str()))
        })
    )
  });

  if is_trigger_char {
    editor.handlers.completions.event(CompletionEvent::TriggerChar {
      cursor,
      doc: doc.id,
      view: view.id,
    });
    return;
  }

  // Check if we should auto-trigger based on word characters
  if !trigger_char_only {
    // Count how many word characters precede the cursor
    let word_char_count = text
      .chars_at(cursor)
      .reversed()
      .take_while(|&ch| char_is_word(ch))
      .count();

    // TODO: Make this configurable (default is 2 in Helix)
    let trigger_length = 2;

    if word_char_count >= trigger_length {
      editor.handlers.completions.event(CompletionEvent::AutoTrigger {
        cursor,
        doc: doc.id,
        view: view.id,
      });
    }
  }
}
