use std::{
  io,
  sync::atomic::{
    self,
    AtomicBool,
  },
  time::SystemTime,
};

use filesentry::EventType;

use crate::{
  EditorConfig,
  core::file_watcher::{
    self,
    FileSystemDidChange,
  },
  doc,
  doc_mut,
  ui::job,
  view_mut,
};

struct AutoReload {
  enable:             AtomicBool,
  prompt_if_modified: AtomicBool,
}

impl AutoReload {
  pub fn refresh_config(&self, config: &EditorConfig) {
    self
      .enable
      .store(config.file_watcher.enable, atomic::Ordering::Relaxed);
    self
      .prompt_if_modified
      .store(config.file_watcher.enable, atomic::Ordering::Relaxed);
  }

      fn on_file_did_change(&self, event: &mut FileSystemDidChange) {
        if !self.enable.load(atomic::Ordering::Relaxed) {
            return;
        }
        let fs_events = event.fs_events.clone();
        if !fs_events
            .iter()
            .any(|event| event.ty == EventType::Modified)
        {
            return;
        }
        job::dispatch_blocking(move |editor, _| {
            let config = editor.config();
            let mut vcs_reload = false;
            for fs_event in &*fs_events {
                if fs_event.ty != EventType::Modified {
                    continue;
                }
                vcs_reload |= editor.diff_providers.needs_reload(fs_event);
                let Some(doc_id) = editor.document_id_by_path(fs_event.path.as_std_path()) else {
                    return;
                };
                let doc = doc_mut!(editor, &doc_id);
                let mtime = match doc.path().unwrap().metadata() {
                    Ok(meta) => meta.modified().unwrap_or(SystemTime::now()),
                    Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                    Err(_) => SystemTime::now(),
                };
                if mtime == doc.last_saved_time {
                    continue;
                }
                if doc.is_modified() {
                    let msg = format!(
                        "{} auto-reload failed due to unsaved changes, use :reload to refresh",
                        doc.relative_path().unwrap().display()
                    );
                    editor.set_warning(msg);
                } else {
                    let scrolloff = config.scrolloff;
                    let view = view_mut!(editor);
                    match doc.reload(view, &editor.diff_providers) {
                        Ok(_) => {
                            view.ensure_cursor_in_view(doc, scrolloff);
                            let msg = format!(
                                "{} auto-reload external changes",
                                doc.relative_path().unwrap().display()
                            );
                            editor.set_status(msg);
                        }
                        Err(err) => {
                            let doc = doc!(editor, &doc_id);
                            let msg = format!(
                                "{} auto-reload failed: {err}",
                                doc.relative_path().unwrap().display()
                            );
                            editor.set_error(msg);
                        }
                    }
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
