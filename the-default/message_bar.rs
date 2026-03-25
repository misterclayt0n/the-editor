use the_lib::messages::MessageDisposition;

use crate::DefaultContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagePresentation {
  InlineStatusline,
  Panel,
  Toast,
  Hidden,
}

fn visible_message<Ctx: DefaultContext>(ctx: &Ctx) -> Option<&the_lib::messages::Message> {
  ctx
    .messages()
    .active()
    .filter(|message| message.disposition != MessageDisposition::Background)
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
