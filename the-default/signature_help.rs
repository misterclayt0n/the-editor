use the_lib::render::{
  LayoutIntent,
  UiConstraints,
  UiContainer,
  UiInsets,
  UiNode,
  UiPanel,
  UiText,
};

use crate::DefaultContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpItem {
  pub label:                  String,
  pub documentation:          Option<String>,
  pub active_parameter:       Option<u32>,
  pub active_parameter_range: Option<std::ops::Range<usize>>,
}

impl SignatureHelpItem {
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      documentation: None,
      active_parameter: None,
      active_parameter_range: None,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct SignatureHelpState {
  pub active:           bool,
  pub signatures:       Vec<SignatureHelpItem>,
  pub active_signature: usize,
  pub docs_scroll:      usize,
}

impl SignatureHelpState {
  pub fn clear(&mut self) {
    self.active = false;
    self.signatures.clear();
    self.active_signature = 0;
    self.docs_scroll = 0;
  }

  pub fn set_signatures(&mut self, signatures: Vec<SignatureHelpItem>, active_signature: usize) {
    self.signatures = signatures;
    self.active = !self.signatures.is_empty();
    self.active_signature = active_signature;
    self.docs_scroll = 0;
    self.clamp();
  }

  pub fn clamp(&mut self) {
    if self.signatures.is_empty() {
      self.clear();
      return;
    }
    self.active_signature = self.active_signature.min(self.signatures.len() - 1);
  }

  pub fn selected(&self) -> Option<&SignatureHelpItem> {
    self
      .signatures
      .get(self.active_signature)
      .or_else(|| self.signatures.first())
  }
}

fn signature_label_markdown(label: &str) -> String {
  format!("```\n{label}\n```")
}

fn build_signature_help_content(
  state: &SignatureHelpState,
  selected: &SignatureHelpItem,
) -> String {
  let mut content = String::new();
  if state.signatures.len() > 1 {
    content.push_str(&format!(
      "({}/{})\n\n",
      state.active_signature + 1,
      state.signatures.len()
    ));
  }

  let signature_label = signature_label_markdown(selected.label.trim());
  content.push_str(signature_label.as_str());

  if let Some(docs) = selected
    .documentation
    .as_deref()
    .map(str::trim)
    .filter(|docs| !docs.is_empty())
  {
    content.push_str("\n\n---\n\n");
    content.push_str(docs);
  }

  content
}

pub fn signature_help_markdown(state: &SignatureHelpState) -> Option<String> {
  if !state.active || state.signatures.is_empty() {
    return None;
  }
  let selected = state.selected()?;
  Some(build_signature_help_content(state, selected))
}

pub fn close_signature_help<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let changed = {
    let Some(state) = ctx.signature_help_mut() else {
      return;
    };
    if !state.active {
      false
    } else {
      state.clear();
      true
    }
  };

  if changed {
    ctx.request_render();
  }
}

pub fn show_signature_help<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  signatures: Vec<SignatureHelpItem>,
  active_signature: usize,
) {
  let changed = {
    let Some(state) = ctx.signature_help_mut() else {
      return;
    };
    state.set_signatures(signatures, active_signature);
    state.active
  };

  if changed {
    ctx.request_render();
  }
}

pub fn signature_help_next<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let changed = {
    let Some(state) = ctx.signature_help_mut() else {
      return;
    };
    if !state.active || state.signatures.is_empty() {
      false
    } else {
      state.active_signature = (state.active_signature + 1) % state.signatures.len();
      state.docs_scroll = 0;
      true
    }
  };

  if changed {
    ctx.request_render();
  }
}

pub fn signature_help_prev<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let changed = {
    let Some(state) = ctx.signature_help_mut() else {
      return;
    };
    if !state.active || state.signatures.is_empty() {
      false
    } else {
      state.active_signature = if state.active_signature == 0 {
        state.signatures.len() - 1
      } else {
        state.active_signature - 1
      };
      state.docs_scroll = 0;
      true
    }
  };

  if changed {
    ctx.request_render();
  }
}

pub fn signature_help_docs_scroll<Ctx: DefaultContext>(ctx: &mut Ctx, delta: isize) {
  let changed = {
    let Some(state) = ctx.signature_help_mut() else {
      return;
    };
    if !state.active || state.signatures.is_empty() {
      false
    } else {
      let next = if delta.is_negative() {
        state.docs_scroll.saturating_sub(delta.unsigned_abs())
      } else {
        state.docs_scroll.saturating_add(delta as usize)
      };
      if next == state.docs_scroll {
        false
      } else {
        state.docs_scroll = next;
        true
      }
    }
  };

  if changed {
    ctx.request_render();
  }
}

pub fn build_signature_help_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Vec<UiNode> {
  let Some(state) = ctx.signature_help_mut() else {
    return Vec::new();
  };
  state.clamp();
  if !state.active || state.signatures.is_empty() {
    return Vec::new();
  }

  let Some(content) = signature_help_markdown(state) else {
    return Vec::new();
  };

  let mut text = UiText::new("signature_help_text", content);
  text.source = Some("signature".to_string());
  text.style = text.style.with_role("completion_docs");
  text.clip = true;

  let mut container = UiContainer::column(
    "signature_help_container",
    0,
    vec![UiNode::Text(text)],
  );
  container.style = container.style.with_role("completion_docs");

  let mut panel = UiPanel::new(
    "signature_help",
    LayoutIntent::Custom("signature_help".to_string()),
    UiNode::Container(container),
  );
  panel.source = Some("signature".to_string());
  panel.style = panel.style.with_role("completion_docs");
  panel.style.border = None;
  panel.constraints = UiConstraints::panel();
  panel.constraints.padding = UiInsets {
    left:   1,
    right:  1,
    top:    1,
    bottom: 1,
  };
  panel.constraints.min_width = Some(12);
  panel.constraints.max_width = Some(72);
  panel.constraints.max_height = Some(16);

  vec![UiNode::Panel(panel)]
}

#[cfg(test)]
mod tests {
  use super::{
    SignatureHelpItem,
    SignatureHelpState,
    build_signature_help_content,
    signature_help_markdown,
    signature_label_markdown,
  };

  #[test]
  fn signature_label_markdown_wraps_signature_in_code_fence() {
    let rendered = signature_label_markdown("add(int x, int y) -> int");
    assert_eq!(rendered, "```\nadd(int x, int y) -> int\n```");
  }

  #[test]
  fn build_signature_help_content_includes_counter_and_docs_separator() {
    let mut state = SignatureHelpState::default();
    let mut first = SignatureHelpItem::new("foo(a: i32, b: i32)");
    first.active_parameter_range = Some(4..10);
    first.documentation = Some("Function docs.".to_string());
    let second = SignatureHelpItem::new("bar()");
    state.set_signatures(vec![first, second], 0);

    let content = build_signature_help_content(&state, state.selected().expect("selected"));
    assert_eq!(
      content,
      "(1/2)\n\n```\nfoo(a: i32, b: i32)\n```\n\n---\n\nFunction docs."
    );
  }

  #[test]
  fn signature_help_markdown_returns_none_when_inactive() {
    let state = SignatureHelpState::default();
    assert!(signature_help_markdown(&state).is_none());
  }
}
