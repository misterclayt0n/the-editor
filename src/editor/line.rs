use std::{
    fmt::Display,
    ops::{Deref, Range},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy)]
enum GraphemeWidth {
    Half,
    Full,
}

type GraphemeIndex = usize;
type ByteIndex = usize;

impl GraphemeWidth {
    const fn saturating_add(self, other: usize) -> usize {
        match self {
            Self::Half => other.saturating_add(1),
            Self::Full => other.saturating_add(2),
        }
    }
}

#[derive(Clone)]
struct TextFragment {
    grapheme: String,
    rendered_width: GraphemeWidth,
    replacement: Option<char>,
    start_byte_index: usize,
}

#[derive(Default, Clone)]
pub struct Line {
    fragments: Vec<TextFragment>,
    string: String,
}

impl Line {
    pub fn from(line_str: &str) -> Self {
        debug_assert!(line_str.is_empty() || line_str.lines().count() == 1);
        let fragments = Self::str_to_fragments(line_str);

        Self {
            fragments,
            string: String::from(line_str),
        }
    }

    pub fn width(&self) -> GraphemeIndex {
        self.width_until(self.grapheme_count())
    }

    pub fn append_char(&mut self, character: char) {
        self.insert_char(character, self.grapheme_count());
    }

    pub fn delete_last(&mut self) {
        self.delete(self.grapheme_count().saturating_sub(1));
    }

    // inserts a character into the line, or appends it at the end if at == grapheme_count + 1
    pub fn insert_char(&mut self, character: char, at: GraphemeIndex) {
        debug_assert!(at.saturating_sub(1) <= self.grapheme_count());

        if let Some(fragment) = self.fragments.get(at) {
            self.string.insert(fragment.start_byte_index, character);
        } else {
            self.string.push(character);
        }

        self.rebuild_fragments();
    }

    pub fn delete(&mut self, at: GraphemeIndex) {
        debug_assert!(at <= self.grapheme_count());

        if let Some(fragment) = self.fragments.get(at) {
            let start = fragment.start_byte_index;
            let end = fragment
                .start_byte_index
                .saturating_add(fragment.grapheme.len());

            self.string.drain(start..end);
            self.rebuild_fragments();
        }
    }

    pub fn append(&mut self, other: &Self) {
        self.string.push_str(&other.string);
        self.rebuild_fragments();
    }

    fn str_to_fragments(line_str: &str) -> Vec<TextFragment> {
        line_str
            .grapheme_indices(true)
            .map(|(byte_index, grapheme)| {
                let (replacement, rendered_width) = Self::get_replacement_char(grapheme)
                    .map_or_else(
                        || {
                            let unicode_width = grapheme.width();
                            let rendered_width = match unicode_width {
                                0 | 1 => GraphemeWidth::Half,
                                _ => GraphemeWidth::Full,
                            };
                            (None, rendered_width)
                        },
                        |replacement| (Some(replacement), GraphemeWidth::Half),
                    );

                TextFragment {
                    grapheme: grapheme.to_string(),
                    rendered_width,
                    replacement,
                    start_byte_index: byte_index,
                }
            })
            .collect()
    }

    fn rebuild_fragments(&mut self) {
        self.fragments = Self::str_to_fragments(&self.string);
    }

    fn get_replacement_char(for_str: &str) -> Option<char> {
        let width = for_str.width();

        match for_str {
            " " => None,
            "\t" => Some(' '),
            _ if width > 0 && for_str.trim().is_empty() => Some('␣'),
            _ if width == 0 => {
                let mut chars = for_str.chars();
                if let Some(ch) = chars.next() {
                    if ch.is_control() && chars.next().is_none() {
                        return Some('▯');
                    }
                }
                Some('·')
            }
            _ => None,
        }
    }

    pub fn get_visible_graphemes(&self, range: Range<GraphemeIndex>) -> String {
        if range.start > range.end {
            return String::new();
        }

        let mut result = String::new();
        let mut current_pos = 0;

        for fragment in &self.fragments {
            let fragment_end = fragment.rendered_width.saturating_add(current_pos);

            if current_pos >= range.end {
                break;
            }

            if fragment_end > range.start {
                if fragment_end > range.end || current_pos < range.start {
                    result.push('⋯');
                } else if let Some(char) = fragment.replacement {
                    result.push(char);
                } else {
                    result.push_str(&fragment.grapheme);
                }
            }

            current_pos = fragment_end;
        }

        return result;
    }

    pub fn grapheme_count(&self) -> GraphemeIndex {
        self.fragments.len()
    }

    pub fn width_until(&self, grapheme_index: GraphemeIndex) -> GraphemeIndex {
        self.fragments
            .iter()
            .take(grapheme_index)
            .map(|fragment| match fragment.rendered_width {
                GraphemeWidth::Half => 1,
                GraphemeWidth::Full => 2,
            })
            .sum()
    }

    pub fn split(&mut self, at: GraphemeIndex) -> Self {
        if let Some(fragment) = self.fragments.get(at) {
            let remainder = self.string.split_off(fragment.start_byte_index);
            self.rebuild_fragments();
            Self::from(&remainder)
        } else {
            Self::default()
        }
    }

    fn byte_index_to_grapheme_index(&self, byte_index: ByteIndex) -> GraphemeIndex {
        debug_assert!(byte_index <= self.string.len());

        self.fragments
            .iter()
            .position(|fragment| fragment.start_byte_index >= byte_index)
            .map_or_else(
                || {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Fragment not found for byte index: {byte_index:?}")
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        0
                    }
                },
                |grapheme_index| grapheme_index,
            )
    }

    fn grapheme_index_to_byte_index(&self, grapheme_index: GraphemeIndex) -> ByteIndex {
        debug_assert!(grapheme_index <= self.grapheme_count());

        if grapheme_index == 0 || self.grapheme_count() == 0 {
            return 0;
        }

        self.fragments.get(grapheme_index).map_or_else(
            || {
                #[cfg(debug_assertions)]
                {
                    panic!("Fragment not found for grapheme index: {grapheme_index:?}")
                }

                #[cfg(not(debug_assertions))]
                {
                    0
                }
            },
            |fragment| fragment.start_byte_index,
        )
    }

    pub fn search_forward(&self, query: &str, from_grapheme_index: GraphemeIndex) -> Option<GraphemeIndex> {
        debug_assert!(from_grapheme_index <= self.grapheme_count());

        if from_grapheme_index == self.grapheme_count() {
            return None;
        }

        let start_byte_index = self.grapheme_index_to_byte_index(from_grapheme_index);

        self.string
            .get(start_byte_index..)
            .and_then(|substring| substring.find(query))
            .map(|byte_index| {
                self.byte_index_to_grapheme_index(byte_index.saturating_add(start_byte_index))
            })
    }

    pub fn search_backward(&self, query: &str, from_grapheme_index: GraphemeIndex) -> Option<GraphemeIndex> {
        debug_assert!(from_grapheme_index <= self.grapheme_count());

        if from_grapheme_index == 0 {
            return None;
        }
        let end_byte_index = if from_grapheme_index == self.grapheme_count() {
            self.string.len()
        } else {
            self.grapheme_index_to_byte_index(from_grapheme_index)
        };
        self.string
            .get(..end_byte_index)
            .and_then(|substr| substr.match_indices(query).last())
            .map(|(index, _)| self.byte_index_to_grapheme_index(index))
    }
}

impl Display for Line {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{}", self.string)
    }
}

impl Deref for Line {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}
