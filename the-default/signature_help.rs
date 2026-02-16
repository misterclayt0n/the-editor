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

  let Some(selected) = state.selected() else {
    return Vec::new();
  };

  let mut content = String::new();
  if state.signatures.len() > 1 {
    content.push_str(&format!(
      "({}/{})\n\n",
      state.active_signature + 1,
      state.signatures.len()
    ));
  }

  content.push_str(selected.label.trim());
  if let Some(docs) = selected
    .documentation
    .as_deref()
    .map(str::trim)
    .filter(|docs| !docs.is_empty())
  {
    content.push_str("\n\n");
    content.push_str(docs);
  }

  let mut text = UiText::new("signature_help_text", content);
  text.source = Some("signature".to_string());
  text.style = text.style.with_role("completion_docs");
  text.clip = false;

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
  panel.constraints.min_width = Some(24);
  panel.constraints.max_width = Some(84);
  panel.constraints.max_height = Some(18);

  vec![UiNode::Panel(panel)]
}
