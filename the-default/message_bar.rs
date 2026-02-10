use the_lib::{
  messages::MessageLevel,
  render::{
    LayoutIntent,
    UiAlign,
    UiAlignPair,
    UiConstraints,
    UiInsets,
    UiLayer,
    UiNode,
    UiPanel,
    UiRadius,
    UiStyle,
    UiText,
  },
};

use crate::DefaultContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagePresentation {
  InlineStatusline,
  Panel,
  Toast,
  Hidden,
}

fn is_lsp_source(source: Option<&str>) -> bool {
  source.is_some_and(|source| source.eq_ignore_ascii_case("lsp"))
}

fn visible_message<Ctx: DefaultContext>(ctx: &Ctx) -> Option<&the_lib::messages::Message> {
  ctx
    .messages()
    .active()
    .filter(|message| !is_lsp_source(message.source.as_deref()))
}

pub fn inline_statusline_message<Ctx: DefaultContext>(ctx: &Ctx) -> Option<String> {
  if !matches!(
    ctx.message_presentation(),
    MessagePresentation::InlineStatusline
  ) {
    return None;
  }

  visible_message(ctx).map(|message| message.text.clone())
}

fn role_for_level(level: MessageLevel) -> &'static str {
  match level {
    MessageLevel::Error => "message_error",
    MessageLevel::Warning => "message_warning",
    MessageLevel::Info => "message_info",
  }
}

pub fn build_message_bar_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Option<UiNode> {
  match ctx.message_presentation() {
    MessagePresentation::Panel => build_panel(ctx),
    MessagePresentation::Toast => build_toast(ctx),
    _ => None,
  }
}

fn build_panel<Ctx: DefaultContext>(ctx: &mut Ctx) -> Option<UiNode> {
  let message = visible_message(ctx)?;
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

fn build_toast<Ctx: DefaultContext>(ctx: &mut Ctx) -> Option<UiNode> {
  let message = visible_message(ctx)?;
  let role = role_for_level(message.level);

  let display = if let Some(source) = message.source.as_deref() {
    format!("{source} \u{2013} {}", message.text)
  } else {
    message.text.clone()
  };

  let mut text = UiText::new("message_toast_text", display);
  text.style = UiStyle::default().with_role(role);
  text.max_lines = Some(1);

  let mut panel = UiPanel::new("message_toast", LayoutIntent::Floating, UiNode::Text(text));
  panel.style = UiStyle::default().with_role(role);
  panel.style.radius = UiRadius::Medium;
  panel.layer = UiLayer::Overlay;
  panel.constraints = UiConstraints {
    min_width:  None,
    max_width:  Some(80),
    min_height: Some(1),
    max_height: Some(1),
    padding:    UiInsets {
      left:   1,
      right:  1,
      top:    0,
      bottom: 0,
    },
    align:      UiAlignPair {
      horizontal: UiAlign::Center,
      vertical:   UiAlign::End,
    },
  };

  Some(UiNode::Panel(panel))
}
