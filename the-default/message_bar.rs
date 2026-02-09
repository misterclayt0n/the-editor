use the_lib::{
  messages::MessageLevel,
  render::{
    LayoutIntent,
    UiConstraints,
    UiInsets,
    UiLayer,
    UiNode,
    UiPanel,
    UiStyle,
    UiText,
  },
};

use crate::DefaultContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagePresentation {
  InlineStatusline,
  Panel,
  Hidden,
}

pub fn inline_statusline_message<Ctx: DefaultContext>(ctx: &Ctx) -> Option<String> {
  if !matches!(
    ctx.message_presentation(),
    MessagePresentation::InlineStatusline
  ) {
    return None;
  }

  ctx.messages().active().map(|message| message.text.clone())
}

fn role_for_level(level: MessageLevel) -> &'static str {
  match level {
    MessageLevel::Error => "message_error",
    MessageLevel::Warning => "message_warning",
    MessageLevel::Info => "message_info",
  }
}

pub fn build_message_bar_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Option<UiNode> {
  if !matches!(ctx.message_presentation(), MessagePresentation::Panel) {
    return None;
  }
  let message = ctx.messages().active()?;
  let role = role_for_level(message.level);

  let mut text = UiText::new("message_bar_text", message.text.clone());
  text.style = UiStyle::default().with_role(role);

  let mut panel = UiPanel::new("message_bar", LayoutIntent::Bottom, UiNode::Text(text));
  panel.style = UiStyle::default().with_role(role);
  panel.style.border = None;
  panel.layer = UiLayer::Background;
  panel.constraints = UiConstraints {
    min_height: Some(1),
    max_height: Some(1),
    padding: UiInsets {
      left:   1,
      right:  1,
      top:    0,
      bottom: 0,
    },
    ..UiConstraints::default()
  };

  Some(UiNode::Panel(panel))
}
