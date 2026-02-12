use ratatui::{
  layout::{
    Constraint,
    Direction,
    Layout,
  },
  prelude::Rect,
  widgets::{
    Block,
    Borders,
  },
};
use the_default::FilePickerState;

#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollbarMetrics {
  pub track:            Rect,
  pub thumb_offset:     u16,
  pub thumb_height:     u16,
  pub max_scroll:       usize,
  pub max_thumb_offset: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FilePickerLayout {
  pub panel:                 Rect,
  pub panel_inner:           Rect,
  pub show_preview:          bool,
  pub list_pane:             Rect,
  pub list_inner:            Rect,
  pub list_prompt:           Rect,
  pub list_area:             Rect,
  pub list_content:          Rect,
  pub list_scroll_offset:    usize,
  pub list_scrollbar_track:  Option<Rect>,
  pub preview_pane:          Option<Rect>,
  pub preview_inner:         Option<Rect>,
  pub preview_content:       Option<Rect>,
  pub preview_scroll_offset: usize,
  pub preview_scrollbar:     Option<Rect>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CompletionDocsLayout {
  pub panel:           Rect,
  pub content:         Rect,
  pub scrollbar_track: Option<Rect>,
  pub visible_rows:    usize,
  pub total_rows:      usize,
}

impl FilePickerLayout {
  pub fn list_visible_rows(self) -> usize {
    self.list_content.height.max(1) as usize
  }

  pub fn preview_visible_rows(self) -> usize {
    self
      .preview_content
      .map(|rect| rect.height.max(1) as usize)
      .unwrap_or(1)
  }
}

pub fn compute_file_picker_layout(
  area: Rect,
  picker: &FilePickerState,
) -> Option<FilePickerLayout> {
  if !picker.active || area.width < 4 || area.height < 4 {
    return None;
  }

  let width = area
    .width
    .saturating_mul(9)
    .saturating_div(10)
    .max(72)
    .min(area.width);
  let height = area
    .height
    .saturating_mul(8)
    .saturating_div(10)
    .max(18)
    .min(area.height);
  let x = area.x + area.width.saturating_sub(width) / 2;
  let y = area.y + area.height.saturating_sub(height) / 2;
  let panel = Rect::new(x, y, width, height);

  let outer = Block::default().borders(Borders::ALL);
  let panel_inner = outer.inner(panel);
  if panel_inner.width < 3 || panel_inner.height < 3 {
    return Some(FilePickerLayout {
      panel,
      panel_inner,
      ..Default::default()
    });
  }

  let show_preview = picker.show_preview && panel_inner.width >= 72;
  let panes = if show_preview {
    Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
      .split(panel_inner)
  } else {
    Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(100)])
      .split(panel_inner)
  };

  let list_pane = panes[0];
  let list_inner = Block::default().borders(Borders::ALL).inner(list_pane);
  let list_prompt = Rect::new(list_inner.x, list_inner.y, list_inner.width, 1);
  let list_area = if list_inner.height >= 3 {
    Rect::new(
      list_inner.x,
      list_inner.y.saturating_add(2),
      list_inner.width,
      list_inner.height.saturating_sub(2),
    )
  } else {
    Rect::default()
  };

  let total_matches = picker.matched_count();
  let visible_rows = list_area.height.max(1) as usize;
  let list_scroll_offset = picker
    .list_offset
    .min(total_matches.saturating_sub(visible_rows));
  let list_scrollbar_track =
    (total_matches > visible_rows && list_area.width > 0 && list_area.height > 0).then(|| {
      Rect::new(
        list_area.x + list_area.width.saturating_sub(1),
        list_area.y,
        1,
        list_area.height,
      )
    });
  let list_content = if list_scrollbar_track.is_some() {
    Rect::new(
      list_area.x,
      list_area.y,
      list_area.width.saturating_sub(1),
      list_area.height,
    )
  } else {
    list_area
  };

  let preview_pane = show_preview.then_some(panes[1]);
  let preview_inner = preview_pane.map(|pane| Block::default().borders(Borders::ALL).inner(pane));
  let preview_total_lines = picker.preview_line_count();
  let preview_visible_rows = preview_inner
    .map(|rect| rect.height.max(1) as usize)
    .unwrap_or(1);
  let preview_scroll_offset = picker
    .preview_scroll
    .min(preview_total_lines.saturating_sub(preview_visible_rows));
  let preview_scrollbar = preview_inner.and_then(|inner| {
    (preview_total_lines > preview_visible_rows && inner.width > 1 && inner.height > 0).then(|| {
      Rect::new(
        inner.x + inner.width.saturating_sub(1),
        inner.y,
        1,
        inner.height,
      )
    })
  });
  let preview_content = preview_inner.map(|inner| {
    if preview_scrollbar.is_some() {
      Rect::new(
        inner.x,
        inner.y,
        inner.width.saturating_sub(1),
        inner.height,
      )
    } else {
      inner
    }
  });

  Some(FilePickerLayout {
    panel,
    panel_inner,
    show_preview,
    list_pane,
    list_inner,
    list_prompt,
    list_area,
    list_content,
    list_scroll_offset,
    list_scrollbar_track,
    preview_pane,
    preview_inner,
    preview_content,
    preview_scroll_offset,
    preview_scrollbar,
  })
}

pub fn compute_scrollbar_metrics(
  track: Rect,
  total_items: usize,
  visible_items: usize,
  scroll_offset: usize,
) -> Option<ScrollbarMetrics> {
  if track.width == 0 || track.height == 0 {
    return None;
  }
  if visible_items == 0 || total_items <= visible_items {
    return None;
  }

  let thumb_height = ((visible_items as f32 / total_items as f32) * track.height as f32)
    .ceil()
    .max(1.0) as u16;
  let thumb_height = thumb_height.min(track.height.max(1));
  let max_scroll = total_items.saturating_sub(visible_items);
  let max_thumb_offset = track.height.saturating_sub(thumb_height);
  let thumb_offset = if max_scroll == 0 || max_thumb_offset == 0 {
    0
  } else {
    ((scroll_offset as f32 / max_scroll as f32) * max_thumb_offset as f32).round() as u16
  };

  Some(ScrollbarMetrics {
    track,
    thumb_offset,
    thumb_height,
    max_scroll,
    max_thumb_offset,
  })
}

pub fn scroll_offset_from_thumb(metrics: ScrollbarMetrics, thumb_offset: u16) -> usize {
  if metrics.max_scroll == 0 || metrics.max_thumb_offset == 0 {
    return 0;
  }
  let thumb_offset = thumb_offset.min(metrics.max_thumb_offset);
  ((thumb_offset as f32 / metrics.max_thumb_offset as f32) * metrics.max_scroll as f32).round()
    as usize
}

pub fn point_in_rect(x: u16, y: u16, rect: Rect) -> bool {
  x >= rect.x
    && y >= rect.y
    && x < rect.x.saturating_add(rect.width)
    && y < rect.y.saturating_add(rect.height)
}
