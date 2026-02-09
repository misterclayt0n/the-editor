use the_lib::render::{
  LayoutIntent,
  UiConstraints,
  UiInsets,
  UiLayer,
  UiNode,
  UiPanel,
  UiStatusBar,
  UiStyle,
};

use crate::{
  DefaultContext,
  Mode,
  message_bar::inline_statusline_message,
};

pub const STATUSLINE_ID: &str = "statusline";

pub fn statusline_present(tree: &the_lib::render::UiTree) -> bool {
  tree.overlays.iter().any(|node| {
    match node {
      UiNode::Panel(panel) => panel.id == STATUSLINE_ID,
      UiNode::StatusBar(status) => status.id.as_deref() == Some(STATUSLINE_ID),
      _ => false,
    }
  })
}

pub fn build_statusline_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> UiNode {
  let doc = ctx.editor_ref().document();
  let slice = doc.text().slice(..);
  let selection = doc.selection();
  let range = selection.ranges().first().copied();

  let (line, col) = if let Some(range) = range {
    let line = range.cursor_line(slice);
    let col = range.cursor(slice).saturating_sub(slice.line_to_char(line));
    (line + 1, col + 1)
  } else {
    (1, 1)
  };

  let file_name = ctx
    .file_path()
    .and_then(|path| path.file_name())
    .and_then(|name| name.to_str())
    .map(str::to_string)
    .unwrap_or_else(|| doc.display_name().to_string());

  let mut left = format!("{}  {}", mode_label(ctx.mode()), file_name);
  let flags = doc.flags();
  if flags.modified {
    left.push_str(" [+]");
  }
  if flags.readonly {
    left.push_str(" [RO]");
  }

  let right = if selection.ranges().len() > 1 {
    format!("{} sel  {line}:{col}", selection.ranges().len())
  } else {
    format!("{line}:{col}")
  };
  let right = if let Some(message) = inline_statusline_message(ctx).filter(|m| !m.is_empty()) {
    format!("{message}  {right}")
  } else {
    right
  };

  let status = UiStatusBar {
    id: Some(STATUSLINE_ID.to_string()),
    left,
    center: String::new(),
    right,
    style: UiStyle::default().with_role("statusline"),
  };

  let mut panel = UiPanel::new(
    STATUSLINE_ID,
    LayoutIntent::Bottom,
    UiNode::StatusBar(status),
  );
  panel.style = UiStyle::default().with_role("statusline");
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

  UiNode::Panel(panel)
}

fn mode_label(mode: Mode) -> &'static str {
  match mode {
    Mode::Normal => "NOR",
    Mode::Insert => "INS",
    Mode::Select => "SEL",
    Mode::Command => "CMD",
  }
}
