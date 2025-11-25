use std::{
  io,
  sync::{
    Arc,
    atomic::{
      self,
      AtomicBool,
    },
  },
  time::SystemTime,
};

use filesentry::EventType;
use the_editor_event::register_hook;

use crate::{
  core::{
    document::Document,
    file_watcher::FileSystemDidChange,
  },
  doc,
  doc_mut,
  editor::EditorConfig,
  event::{
    ConfigDidChange,
    DocumentDidClose,
    DocumentDidOpen,
  },
  ui::job,
  view_mut,
};

pub(crate) fn register_hooks(config: &EditorConfig) {
  let handler = Arc::new(AutoReload::new(config));

  let handler_cfg = handler.clone();
  register_hook!(move |event: &mut ConfigDidChange<'_>| {
    handler_cfg.refresh_config(event.new);
    Ok(())
  });

  let handler_fs = handler.clone();
  register_hook!(move |event: &mut FileSystemDidChange| {
    handler_fs.on_file_did_change(event);
    Ok(())
  });

  register_hook!(move |event: &mut DocumentDidOpen<'_>| {
    let path = {
      let doc = doc!(event.editor, &event.doc);
      doc.path().cloned()
    };
    if let Some(path) = path {
      event.editor.watch_document_path(path.as_path());
    }
    Ok(())
  });

  register_hook!(move |event: &mut DocumentDidClose<'_>| {
    if let Some(path) = event.doc.path().cloned() {
      event.editor.unwatch_document_path(path.as_path());
    }
    Ok(())
  });
}

struct AutoReload {
  enable:             AtomicBool,
  prompt_if_modified: AtomicBool,
}

impl AutoReload {
  fn new(config: &EditorConfig) -> Self {
    Self {
      enable:             AtomicBool::new(config.auto_reload.enable),
      prompt_if_modified: AtomicBool::new(config.auto_reload.prompt_if_modified),
    }
  }

  fn refresh_config(&self, config: &EditorConfig) {
    self
      .enable
      .store(config.auto_reload.enable, atomic::Ordering::Relaxed);
    self.prompt_if_modified.store(
      config.auto_reload.prompt_if_modified,
      atomic::Ordering::Relaxed,
    );
  }

  fn on_file_did_change(&self, event: &mut FileSystemDidChange) {
    if !self.enable.load(atomic::Ordering::Relaxed) {
      return;
    }

    let fs_events = event.fs_events.clone();
    if !fs_events.iter().any(|evt| evt.ty == EventType::Modified) {
      return;
    }

    let prompt_if_modified = self.prompt_if_modified.load(atomic::Ordering::Relaxed);
    job::dispatch_blocking(move |editor, _| {
      // Skip auto-reload if there are pending saves to avoid race conditions.
      // The file watcher may detect our own save before the save event is processed,
      // which would cause a spurious reload.
      if editor.write_count > 0 {
        return;
      }

      let scrolloff = editor.config().scrolloff;
      let mut vcs_reload = false;

      for fs_event in &*fs_events {
        if fs_event.ty != EventType::Modified {
          continue;
        }

        vcs_reload |= editor.diff_providers.needs_reload(fs_event);

        let Some(doc_id) = editor.document_id_by_path(fs_event.path.as_std_path()) else {
          continue;
        };

        enum ReloadDecision {
          Skip,
          Warn(String),
          Reload(String),
        }

        let decision = 'decision: {
          let doc = doc_mut!(editor, &doc_id);
          let Some(doc_path) = doc.path().cloned() else {
            break 'decision ReloadDecision::Skip;
          };

          let mtime = match doc_path.metadata() {
            Ok(meta) => meta.modified().unwrap_or_else(|_| SystemTime::now()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
              break 'decision ReloadDecision::Skip;
            },
            Err(_) => SystemTime::now(),
          };

          if mtime == doc.last_saved_time {
            break 'decision ReloadDecision::Skip;
          }

          let label = document_display_name(doc);
          if doc.is_modified() {
            let message = if prompt_if_modified {
              format!("{label} has unsaved changes, use :reload to discard them")
            } else {
              format!("{label} auto-reload failed due to unsaved changes, use :reload to refresh")
            };
            break 'decision ReloadDecision::Warn(message);
          }

          break 'decision ReloadDecision::Reload(label);
        };

        let label = match decision {
          ReloadDecision::Skip => continue,
          ReloadDecision::Warn(message) => {
            editor.set_warning(message);
            continue;
          },
          ReloadDecision::Reload(label) => label,
        };

        let reload_result = {
          let view = view_mut!(editor);
          let doc = doc_mut!(editor, &doc_id);
          let result = doc.reload(view, &editor.diff_providers);
          if result.is_ok() {
            view.ensure_cursor_in_view(doc, scrolloff);
          }
          result
        };

        match reload_result {
          Ok(_) => {
            editor.set_status(format!("{label} auto-reload external changes"));
          },
          Err(err) => {
            editor.set_error(format!("{label} auto-reload failed: {err}"));
          },
        }
      }

      if vcs_reload {
        for doc in editor.documents.values_mut() {
          let Some(path) = doc.path() else {
            continue;
          };
          match editor.diff_providers.get_diff_base(path) {
            Some(diff_base) => doc.set_diff_base(diff_base),
            None => doc.diff_handle = None,
          }
        }
      }
    });
  }
}

fn document_display_name(doc: &Document) -> String {
  doc
    .relative_path()
    .map(|path| path.display().to_string())
    .or_else(|| doc.path().map(|path| path.display().to_string()))
    .unwrap_or_else(|| "untitled buffer".to_string())
}
