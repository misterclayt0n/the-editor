use std::collections::VecDeque;

use serde::{
  Deserialize,
  Serialize,
};

pub const DEFAULT_HISTORY_LIMIT: usize = 256;
pub const DEFAULT_EVENT_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageLevel {
  Info,
  Warning,
  Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
  pub id:     u64,
  pub level:  MessageLevel,
  pub source: Option<String>,
  pub text:   String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageEventKind {
  Published { message: Message },
  Dismissed { id: u64 },
  Cleared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEvent {
  pub seq:  u64,
  #[serde(flatten)]
  pub kind: MessageEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageSnapshot {
  pub active:     Option<Message>,
  pub oldest_seq: u64,
  pub latest_seq: u64,
}

#[derive(Debug, Clone)]
pub struct MessageCenter {
  active:          Option<Message>,
  history:         VecDeque<Message>,
  events:          VecDeque<MessageEvent>,
  next_message_id: u64,
  next_event_seq:  u64,
  history_limit:   usize,
  event_limit:     usize,
}

impl Default for MessageCenter {
  fn default() -> Self {
    Self::with_limits(DEFAULT_HISTORY_LIMIT, DEFAULT_EVENT_LIMIT)
  }
}

impl MessageCenter {
  pub fn with_limits(history_limit: usize, event_limit: usize) -> Self {
    Self {
      active:          None,
      history:         VecDeque::new(),
      events:          VecDeque::new(),
      next_message_id: 1,
      next_event_seq:  1,
      history_limit:   history_limit.max(1),
      event_limit:     event_limit.max(1),
    }
  }

  pub fn active(&self) -> Option<&Message> {
    self.active.as_ref()
  }

  pub fn history_len(&self) -> usize {
    self.history.len()
  }

  pub fn history(&self) -> impl Iterator<Item = &Message> {
    self.history.iter()
  }

  pub fn latest_seq(&self) -> u64 {
    self.next_event_seq.saturating_sub(1)
  }

  pub fn oldest_seq(&self) -> u64 {
    self
      .events
      .front()
      .map(|event| event.seq)
      .unwrap_or(self.next_event_seq)
  }

  pub fn snapshot(&self) -> MessageSnapshot {
    MessageSnapshot {
      active:     self.active.clone(),
      oldest_seq: self.oldest_seq(),
      latest_seq: self.latest_seq(),
    }
  }

  pub fn events_since(&self, seq: u64) -> Vec<MessageEvent> {
    self
      .events
      .iter()
      .filter(|event| event.seq > seq)
      .cloned()
      .collect()
  }

  pub fn publish(
    &mut self,
    level: MessageLevel,
    source: Option<String>,
    text: impl Into<String>,
  ) -> Message {
    let message = Message {
      id: self.next_message_id,
      level,
      source,
      text: text.into(),
    };
    self.next_message_id = self.next_message_id.saturating_add(1);

    // A background message (source="lsp") must not displace an active
    // foreground message.  It still goes into history and events.
    let new_is_bg = is_background_source(message.source.as_deref());
    let active_is_fg = self
      .active
      .as_ref()
      .is_some_and(|m| !is_background_source(m.source.as_deref()));
    if !(new_is_bg && active_is_fg) {
      self.active = Some(message.clone());
    }

    self.history.push_back(message.clone());
    while self.history.len() > self.history_limit {
      self.history.pop_front();
    }

    self.push_event(MessageEventKind::Published {
      message: message.clone(),
    });
    message
  }

  pub fn info(&mut self, source: Option<String>, text: impl Into<String>) -> Message {
    self.publish(MessageLevel::Info, source, text)
  }

  pub fn warning(&mut self, source: Option<String>, text: impl Into<String>) -> Message {
    self.publish(MessageLevel::Warning, source, text)
  }

  pub fn error(&mut self, source: Option<String>, text: impl Into<String>) -> Message {
    self.publish(MessageLevel::Error, source, text)
  }

  pub fn dismiss_active(&mut self) -> Option<Message> {
    let message = self.active.take();
    if let Some(message) = message.as_ref() {
      self.push_event(MessageEventKind::Dismissed { id: message.id });
    }
    message
  }

  pub fn clear(&mut self) {
    let changed =
      self.active.take().is_some() || !self.history.is_empty() || !self.events.is_empty();
    self.history.clear();
    if changed {
      self.push_event(MessageEventKind::Cleared);
    }
  }

  fn push_event(&mut self, kind: MessageEventKind) {
    let event = MessageEvent {
      seq: self.next_event_seq,
      kind,
    };
    self.next_event_seq = self.next_event_seq.saturating_add(1);
    self.events.push_back(event);
    while self.events.len() > self.event_limit {
      self.events.pop_front();
    }
  }
}

fn is_background_source(source: Option<&str>) -> bool {
  source.is_some_and(|s| s.eq_ignore_ascii_case("lsp"))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn publish_sets_active_and_emits_event() {
    let mut center = MessageCenter::default();
    let message = center.publish(MessageLevel::Error, Some("test".to_string()), "boom");
    assert_eq!(center.active(), Some(&message));
    assert_eq!(center.history_len(), 1);

    let events = center.events_since(0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].seq, 1);
    assert!(matches!(events[0].kind, MessageEventKind::Published { .. }));
  }

  #[test]
  fn history_and_event_limits_are_enforced() {
    let mut center = MessageCenter::with_limits(2, 2);
    center.info(None, "a");
    center.info(None, "b");
    center.info(None, "c");
    assert_eq!(center.history_len(), 2);

    let events = center.events_since(0);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].seq, 2);
    assert_eq!(events[1].seq, 3);
  }

  #[test]
  fn lsp_message_does_not_displace_editor_active() {
    let mut center = MessageCenter::default();

    // Editor message becomes active.
    let editor_msg = center.info(Some("save".into()), "File saved");
    assert_eq!(center.active().unwrap().text, "File saved");

    // LSP message must not displace it.
    center.info(Some("lsp".into()), "cargo check");
    assert_eq!(center.active().unwrap().id, editor_msg.id);

    // But it still goes into history.
    assert_eq!(center.history_len(), 2);
  }

  #[test]
  fn lsp_message_replaces_lsp_active() {
    let mut center = MessageCenter::default();
    center.info(Some("lsp".into()), "starting");
    center.info(Some("lsp".into()), "cargo check");
    assert_eq!(center.active().unwrap().text, "cargo check");
  }

  #[test]
  fn editor_message_replaces_lsp_active() {
    let mut center = MessageCenter::default();
    center.info(Some("lsp".into()), "cargo check");
    center.info(Some("save".into()), "File saved");
    assert_eq!(center.active().unwrap().text, "File saved");
  }
}
