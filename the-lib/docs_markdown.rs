use std::ops::Range;

use pulldown_cmark::{
  CodeBlockKind,
  Event,
  HeadingLevel,
  Options,
  Parser,
  Tag,
  TagEnd,
};
use serde::{
  Deserialize,
  Serialize,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocsSemanticKind {
  Body,
  Heading1,
  Heading2,
  Heading3,
  Heading4,
  Heading5,
  Heading6,
  ListMarker,
  QuoteMarker,
  QuoteText,
  Link,
  InlineCode,
  Code,
  ActiveParameter,
  Rule,
}

impl DocsSemanticKind {
  pub fn from_heading_level(level: u8) -> Self {
    match level {
      1 => Self::Heading1,
      2 => Self::Heading2,
      3 => Self::Heading3,
      4 => Self::Heading4,
      5 => Self::Heading5,
      _ => Self::Heading6,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocsInlineKind {
  Text,
  Link,
  InlineCode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocsInlineRun {
  pub text:             String,
  pub kind:             DocsInlineKind,
  pub link_destination: Option<String>,
  pub strong:           bool,
  pub emphasis:         bool,
  pub strikethrough: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocsListMarker {
  Bullet,
  Ordered(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocsBlock {
  Paragraph(Vec<DocsInlineRun>),
  Heading {
    level: u8,
    runs:  Vec<DocsInlineRun>,
  },
  ListItem {
    marker: DocsListMarker,
    runs:   Vec<DocsInlineRun>,
  },
  Quote(Vec<DocsInlineRun>),
  CodeFence {
    language: Option<String>,
    lines:    Vec<String>,
  },
  Rule,
  BlankLine,
}

#[derive(Clone, Copy, Debug)]
enum ActiveBlockKind {
  Paragraph,
  Heading(u8),
  ListItem,
  Quote,
}

#[derive(Clone, Debug)]
struct ActiveBlock {
  kind:   ActiveBlockKind,
  marker: Option<DocsListMarker>,
  runs:   Vec<DocsInlineRun>,
  start:  usize,
  end:    usize,
}

#[derive(Clone, Debug)]
struct ActiveCodeBlock {
  language: Option<String>,
  text:     String,
  start:    usize,
  end:      usize,
}

#[derive(Clone, Debug)]
struct SpannedDocsBlock {
  block: DocsBlock,
  span:  Range<usize>,
}

#[derive(Clone, Copy, Debug, Default)]
struct InlineState {
  strong_depth:       u8,
  emphasis_depth:     u8,
  strikethrough_depth: u8,
}

#[derive(Clone, Copy, Debug)]
struct ListState {
  next_ordinal: Option<u64>,
}

fn heading_level_number(level: HeadingLevel) -> u8 {
  match level {
    HeadingLevel::H1 => 1,
    HeadingLevel::H2 => 2,
    HeadingLevel::H3 => 3,
    HeadingLevel::H4 => 4,
    HeadingLevel::H5 => 5,
    HeadingLevel::H6 => 6,
  }
}

fn docs_parse_markdown_fence_language(info: &str) -> Option<String> {
  let token = info
    .trim()
    .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | '{' | '}'))
    .next()
    .unwrap_or_default()
    .trim_matches('.')
    .to_ascii_lowercase();
  (!token.is_empty()).then_some(token)
}

fn docs_code_lines(text: &str) -> Vec<String> {
  let normalized = text.replace('\t', "  ");
  let mut lines = normalized
    .split('\n')
    .map(|line| line.trim_end_matches('\r').to_string())
    .collect::<Vec<_>>();
  if normalized.ends_with('\n') {
    lines.pop();
  }
  if lines.is_empty() {
    lines.push(String::new());
  }
  lines
}

fn docs_blank_lines_between(segment: &str) -> usize {
  if segment.is_empty() {
    return 0;
  }
  let normalized = segment.replace("\r\n", "\n");
  let parts = normalized.split('\n').collect::<Vec<_>>();
  if parts.len() <= 2 {
    return 0;
  }
  parts[1..parts.len() - 1]
    .iter()
    .filter(|line| line.trim().is_empty())
    .count()
}

fn docs_push_inline_run(
  runs: &mut Vec<DocsInlineRun>,
  text: String,
  kind: DocsInlineKind,
  inline_state: InlineState,
  link_destination: Option<&str>,
) {
  if text.is_empty() {
    return;
  }
  let run = DocsInlineRun {
    text,
    kind,
    link_destination: link_destination.map(str::to_string),
    strong: inline_state.strong_depth > 0,
    emphasis: inline_state.emphasis_depth > 0,
    strikethrough: inline_state.strikethrough_depth > 0,
  };
  if let Some(last) = runs.last_mut()
    && last.kind == run.kind
    && last.link_destination == run.link_destination
    && last.strong == run.strong
    && last.emphasis == run.emphasis
    && last.strikethrough == run.strikethrough
  {
    last.text.push_str(&run.text);
    return;
  }
  runs.push(run);
}

fn docs_next_list_marker(list_stack: &mut [ListState]) -> DocsListMarker {
  let Some(state) = list_stack.last_mut() else {
    return DocsListMarker::Bullet;
  };
  if let Some(next) = state.next_ordinal {
    let marker = DocsListMarker::Ordered(format!("{next}."));
    state.next_ordinal = Some(next.saturating_add(1));
    marker
  } else {
    DocsListMarker::Bullet
  }
}

fn docs_begin_active_block(
  active_block: &mut Option<ActiveBlock>,
  kind: ActiveBlockKind,
  marker: Option<DocsListMarker>,
  start: usize,
) {
  if active_block.is_some() {
    return;
  }
  *active_block = Some(ActiveBlock {
    kind,
    marker,
    runs: Vec::new(),
    start,
    end: start,
  });
}

fn docs_finish_active_block(
  blocks: &mut Vec<SpannedDocsBlock>,
  active_block: &mut Option<ActiveBlock>,
  fallback_end: usize,
) {
  let Some(mut block) = active_block.take() else {
    return;
  };
  if block.end <= block.start {
    block.end = fallback_end.max(block.start);
  }
  let span = block.start..block.end;
  let docs_block = match block.kind {
    ActiveBlockKind::Paragraph if !block.runs.is_empty() => DocsBlock::Paragraph(block.runs),
    ActiveBlockKind::Heading(level) => DocsBlock::Heading {
      level,
      runs: block.runs,
    },
    ActiveBlockKind::ListItem => DocsBlock::ListItem {
      marker: block.marker.unwrap_or(DocsListMarker::Bullet),
      runs:   block.runs,
    },
    ActiveBlockKind::Quote => DocsBlock::Quote(block.runs),
    _ => return,
  };
  blocks.push(SpannedDocsBlock {
    block: docs_block,
    span,
  });
}

fn docs_finish_active_code_block(
  blocks: &mut Vec<SpannedDocsBlock>,
  active_code_block: &mut Option<ActiveCodeBlock>,
  fallback_end: usize,
) {
  let Some(mut code_block) = active_code_block.take() else {
    return;
  };
  if code_block.end <= code_block.start {
    code_block.end = fallback_end.max(code_block.start);
  }
  blocks.push(SpannedDocsBlock {
    block: DocsBlock::CodeFence {
      language: code_block.language,
      lines:    docs_code_lines(&code_block.text),
    },
    span:  code_block.start..code_block.end,
  });
}

pub fn parse_markdown_blocks(markdown: &str) -> Vec<DocsBlock> {
  let mut blocks: Vec<SpannedDocsBlock> = Vec::new();
  let mut active_block: Option<ActiveBlock> = None;
  let mut active_code_block: Option<ActiveCodeBlock> = None;
  let mut inline_state = InlineState::default();
  let mut link_targets: Vec<String> = Vec::new();
  let mut list_stack: Vec<ListState> = Vec::new();
  let mut pending_item_marker: Option<DocsListMarker> = None;
  let mut heading_level: Option<u8> = None;
  let mut block_quote_depth = 0usize;

  let parser = Parser::new_ext(markdown, Options::all()).into_offset_iter();
  for (event, range) in parser {
    match event {
      Event::Start(tag) => match tag {
        Tag::Paragraph => {
          let (kind, marker) = if let Some(marker) = pending_item_marker.take() {
            (ActiveBlockKind::ListItem, Some(marker))
          } else if let Some(level) = heading_level {
            (ActiveBlockKind::Heading(level), None)
          } else if block_quote_depth > 0 {
            (ActiveBlockKind::Quote, None)
          } else {
            (ActiveBlockKind::Paragraph, None)
          };
          docs_begin_active_block(&mut active_block, kind, marker, range.start);
        },
        Tag::Heading { level, .. } => {
          docs_finish_active_block(&mut blocks, &mut active_block, range.start);
          heading_level = Some(heading_level_number(level));
          docs_begin_active_block(
            &mut active_block,
            ActiveBlockKind::Heading(heading_level.unwrap_or(1)),
            None,
            range.start,
          );
        },
        Tag::BlockQuote(_) => {
          block_quote_depth = block_quote_depth.saturating_add(1);
        },
        Tag::List(start) => {
          list_stack.push(ListState {
            next_ordinal: start,
          });
        },
        Tag::Item => {
          pending_item_marker = Some(docs_next_list_marker(&mut list_stack));
        },
        Tag::CodeBlock(kind) => {
          docs_finish_active_block(&mut blocks, &mut active_block, range.start);
          let language = match kind {
            CodeBlockKind::Fenced(info) => docs_parse_markdown_fence_language(info.as_ref()),
            CodeBlockKind::Indented => None,
          };
          active_code_block = Some(ActiveCodeBlock {
            language,
            text: String::new(),
            start: range.start,
            end: range.start,
          });
        },
        Tag::Emphasis => {
          inline_state.emphasis_depth = inline_state.emphasis_depth.saturating_add(1);
        },
        Tag::Strong => {
          inline_state.strong_depth = inline_state.strong_depth.saturating_add(1);
        },
        Tag::Strikethrough => {
          inline_state.strikethrough_depth = inline_state.strikethrough_depth.saturating_add(1);
        },
        Tag::Link { dest_url, .. } => {
          link_targets.push(dest_url.to_string());
        },
        _ => {},
      },
      Event::End(tag) => match tag {
        TagEnd::Paragraph => {
          docs_finish_active_block(&mut blocks, &mut active_block, range.end);
        },
        TagEnd::Heading(_) => {
          docs_finish_active_block(&mut blocks, &mut active_block, range.end);
          heading_level = None;
        },
        TagEnd::BlockQuote(_) => {
          docs_finish_active_block(&mut blocks, &mut active_block, range.end);
          block_quote_depth = block_quote_depth.saturating_sub(1);
        },
        TagEnd::List(_) => {
          list_stack.pop();
        },
        TagEnd::Item => {
          docs_finish_active_block(&mut blocks, &mut active_block, range.end);
          pending_item_marker = None;
        },
        TagEnd::CodeBlock => {
          docs_finish_active_code_block(&mut blocks, &mut active_code_block, range.end);
        },
        TagEnd::Emphasis => {
          inline_state.emphasis_depth = inline_state.emphasis_depth.saturating_sub(1);
        },
        TagEnd::Strong => {
          inline_state.strong_depth = inline_state.strong_depth.saturating_sub(1);
        },
        TagEnd::Strikethrough => {
          inline_state.strikethrough_depth = inline_state.strikethrough_depth.saturating_sub(1);
        },
        TagEnd::Link => {
          link_targets.pop();
        },
        _ => {},
      },
      Event::Text(text) => {
        if let Some(code_block) = active_code_block.as_mut() {
          code_block.text.push_str(text.as_ref());
          code_block.end = range.end;
          continue;
        }
        if active_block.is_none() {
          let (kind, marker) = if let Some(marker) = pending_item_marker.take() {
            (ActiveBlockKind::ListItem, Some(marker))
          } else if let Some(level) = heading_level {
            (ActiveBlockKind::Heading(level), None)
          } else if block_quote_depth > 0 {
            (ActiveBlockKind::Quote, None)
          } else {
            (ActiveBlockKind::Paragraph, None)
          };
          docs_begin_active_block(&mut active_block, kind, marker, range.start);
        }
        if let Some(block) = active_block.as_mut() {
          let link_destination = link_targets.last().map(String::as_str);
          let kind = if link_destination.is_some() {
            DocsInlineKind::Link
          } else {
            DocsInlineKind::Text
          };
          docs_push_inline_run(
            &mut block.runs,
            text.into_string(),
            kind,
            inline_state,
            link_destination,
          );
          block.end = range.end;
        }
      },
      Event::Code(text) => {
        if active_block.is_none() {
          let (kind, marker) = if let Some(marker) = pending_item_marker.take() {
            (ActiveBlockKind::ListItem, Some(marker))
          } else if let Some(level) = heading_level {
            (ActiveBlockKind::Heading(level), None)
          } else if block_quote_depth > 0 {
            (ActiveBlockKind::Quote, None)
          } else {
            (ActiveBlockKind::Paragraph, None)
          };
          docs_begin_active_block(&mut active_block, kind, marker, range.start);
        }
        if let Some(block) = active_block.as_mut() {
          docs_push_inline_run(
            &mut block.runs,
            text.into_string(),
            DocsInlineKind::InlineCode,
            inline_state,
            None,
          );
          block.end = range.end;
        }
      },
      Event::SoftBreak | Event::HardBreak => {
        if let Some(code_block) = active_code_block.as_mut() {
          code_block.text.push('\n');
          code_block.end = range.end;
        } else if let Some(block) = active_block.as_mut() {
          docs_push_inline_run(
            &mut block.runs,
            " ".to_string(),
            DocsInlineKind::Text,
            inline_state,
            None,
          );
          block.end = range.end;
        }
      },
      Event::Rule => {
        docs_finish_active_block(&mut blocks, &mut active_block, range.start);
        blocks.push(SpannedDocsBlock {
          block: DocsBlock::Rule,
          span:  range,
        });
      },
      Event::Html(text) | Event::InlineHtml(text) => {
        if active_block.is_none() {
          docs_begin_active_block(&mut active_block, ActiveBlockKind::Paragraph, None, range.start);
        }
        if let Some(block) = active_block.as_mut() {
          docs_push_inline_run(
            &mut block.runs,
            text.into_string(),
            DocsInlineKind::Text,
            inline_state,
            None,
          );
          block.end = range.end;
        }
      },
      Event::FootnoteReference(text) => {
        if active_block.is_none() {
          docs_begin_active_block(&mut active_block, ActiveBlockKind::Paragraph, None, range.start);
        }
        if let Some(block) = active_block.as_mut() {
          docs_push_inline_run(
            &mut block.runs,
            text.into_string(),
            DocsInlineKind::Text,
            inline_state,
            None,
          );
          block.end = range.end;
        }
      },
      Event::TaskListMarker(checked) => {
        if active_block.is_none() {
          let marker = pending_item_marker.take();
          docs_begin_active_block(
            &mut active_block,
            ActiveBlockKind::ListItem,
            marker,
            range.start,
          );
        }
        if let Some(block) = active_block.as_mut() {
          let marker = if checked { "[x] " } else { "[ ] " };
          docs_push_inline_run(
            &mut block.runs,
            marker.to_string(),
            DocsInlineKind::Text,
            inline_state,
            None,
          );
          block.end = range.end;
        }
      },
      _ => {},
    }
  }

  docs_finish_active_block(&mut blocks, &mut active_block, markdown.len());
  docs_finish_active_code_block(&mut blocks, &mut active_code_block, markdown.len());

  let mut out = Vec::new();
  let mut last_end = None;
  for block in blocks {
    if let Some(end) = last_end
      && end <= block.span.start
      && block.span.start <= markdown.len()
    {
      let segment = &markdown[end..block.span.start];
      let blank_lines = docs_blank_lines_between(segment);
      for _ in 0..blank_lines {
        out.push(DocsBlock::BlankLine);
      }
    }
    out.push(block.block);
    last_end = Some(block.span.end.min(markdown.len()));
  }
  out
}

pub fn language_filename_hints(marker: &str) -> Vec<String> {
  let marker = marker.trim().trim_matches('.').to_ascii_lowercase();
  let mut out = Vec::new();
  let mut push_unique = |value: &str| {
    if value.is_empty() || out.iter().any(|existing| existing == value) {
      return;
    }
    out.push(value.to_string());
  };

  push_unique(marker.as_str());
  match marker.as_str() {
    "rust" => push_unique("rs"),
    "javascript" | "js" => push_unique("js"),
    "typescript" | "ts" => push_unique("ts"),
    "python" | "py" => push_unique("py"),
    "shell" | "bash" | "sh" | "zsh" => push_unique("sh"),
    "c++" | "cpp" | "cc" | "cxx" => push_unique("cpp"),
    "c#" | "csharp" => push_unique("cs"),
    "objective-c" | "objc" => push_unique("m"),
    "objective-cpp" | "objcpp" => push_unique("mm"),
    "markdown" => push_unique("md"),
    "yaml" => push_unique("yml"),
    _ => {},
  }
  out
}

#[cfg(test)]
mod tests {
  use super::{
    DocsBlock,
    DocsInlineKind,
    DocsListMarker,
    language_filename_hints,
    parse_markdown_blocks,
  };

  fn flatten_text(block: &DocsBlock) -> String {
    match block {
      DocsBlock::Paragraph(runs) | DocsBlock::Quote(runs) => {
        runs.iter().map(|run| run.text.as_str()).collect::<String>()
      },
      DocsBlock::Heading { runs, .. } => runs.iter().map(|run| run.text.as_str()).collect(),
      DocsBlock::ListItem { runs, .. } => runs.iter().map(|run| run.text.as_str()).collect(),
      DocsBlock::CodeFence { lines, .. } => lines.join("\n"),
      DocsBlock::Rule => "rule".to_string(),
      DocsBlock::BlankLine => String::new(),
    }
  }

  #[test]
  fn markdown_blocks_parse_headings_lists_links_and_code() {
    let blocks = parse_markdown_blocks(
      "# Title\n\n- item\n\n[Result](https://example.com)\n\n```rs\nfn test() {}\n```",
    );
    assert!(matches!(
      blocks.first(),
      Some(DocsBlock::Heading { level: 1, .. })
    ));
    assert!(blocks
      .iter()
      .any(|block| matches!(block, DocsBlock::ListItem { marker: DocsListMarker::Bullet, .. })));
    assert!(blocks.iter().any(|block| matches!(block, DocsBlock::CodeFence { .. })));
    let joined = blocks
      .iter()
      .map(flatten_text)
      .filter(|line| !line.is_empty())
      .collect::<Vec<_>>();
    assert!(joined.iter().any(|line| line == "Title"));
    assert!(joined.iter().any(|line| line == "item"));
    assert!(joined.iter().any(|line| line == "Result"));
    assert!(joined.iter().any(|line| line == "fn test() {}"));
  }

  #[test]
  fn markdown_blocks_keep_literal_escapes_as_text() {
    let blocks = parse_markdown_blocks("\\[x\\] \\*literal\\*");
    let Some(DocsBlock::Paragraph(runs)) = blocks.first() else {
      panic!("expected paragraph block");
    };
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].kind, DocsInlineKind::Text);
    assert_eq!(runs[0].text, "[x] *literal*");
  }

  #[test]
  fn markdown_blocks_capture_link_destination() {
    let blocks = parse_markdown_blocks("[fmt docs](https://pkg.go.dev/fmt)");
    let Some(DocsBlock::Paragraph(runs)) = blocks.first() else {
      panic!("expected paragraph block");
    };
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].kind, DocsInlineKind::Link);
    assert_eq!(runs[0].text, "fmt docs");
    assert_eq!(
      runs[0].link_destination.as_deref(),
      Some("https://pkg.go.dev/fmt")
    );
  }

  #[test]
  fn markdown_blocks_preserve_blank_line_separators() {
    let blocks = parse_markdown_blocks("a\n\nb");
    assert_eq!(blocks.len(), 3);
    assert!(matches!(blocks[1], DocsBlock::BlankLine));
  }

  #[test]
  fn language_hints_include_rust_extension_alias() {
    let hints = language_filename_hints("rust");
    assert!(hints.iter().any(|hint| hint == "rs"));
  }
}
