use std::{cmp, ops::Range};
use unicode_segmentation::UnicodeSegmentation;

pub struct Line {
    graphemes: Vec<String>,
}

impl Line {
    pub fn from(line_str: &str) -> Self {
        let graphemes: Vec<String> = line_str.graphemes(true).map(String::from).collect();

        Self { graphemes }
    }

    pub fn get(&self, range: Range<usize>) -> String {
        let start = range.start;
        let end = cmp::min(range.end, self.graphemes.len());

        self.graphemes[start..end].concat()
    }

    pub fn len(&self) -> usize {
        self.graphemes.len()
    }
}
