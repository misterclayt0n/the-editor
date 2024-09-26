use std::{
    cmp::{max, min},
    fmt::{self, Display},
};

pub mod annotationtype;
pub use annotationtype::AnnotationType;
mod annotation;
use annotation::Annotation;
mod annotatedstringpart;
use annotatedstringpart::AnnotatedStringPart;
mod annotatedstringiterator;
use annotatedstringiterator::AnnotatedStringIterator;

use super::ByteIndex;

#[derive(Default, Debug)]
pub struct AnnotatedString {
    string: String,
    annotations: Vec<Annotation>,
}

impl AnnotatedString {
    pub fn from(string: &str) -> Self {
        Self {
            string: String::from(string),
            annotations: Vec::new(),
        }
    }

    pub fn add_annotation(
        &mut self,
        annotation_type: AnnotationType,
        start: ByteIndex,
        end: ByteIndex,
    ) {
        debug_assert!(start <= end);
        self.annotations.push(Annotation {
            annotation_type,
            start,
            end,
        });
    }

    pub fn truncate_left_until(&mut self, until: ByteIndex) {
        self.replace(0, until, "");
    }

    pub fn truncate_right_from(&mut self, from: ByteIndex) {
        self.replace(from, self.string.len(), "");
    }

    pub fn replace(&mut self, start: ByteIndex, end: ByteIndex, new_string: &str) {
        let end = min(end, self.string.len());

        debug_assert!(start <= end);
        debug_assert!(start <= self.string.len());

        if start > end {
            return;
        }

        self.string.replace_range(start..end, new_string);

        let replaced_range_len = end.saturating_sub(start); // This is the range we want to replace.
        let shortened = new_string.len() < replaced_range_len;
        let len_difference = new_string.len().abs_diff(replaced_range_len); // This is how much longer or shorter the new range is.

        if len_difference == 0 {
            // No adjustment of annotations needed in case the replacement did not result in a change in length.
            return;
        }

        self.annotations.iter_mut().for_each(|annotation| {
            annotation.start = if annotation.start >= end {
                // For annotations starting after the replaced range, we move the start index by the difference in length.
                if shortened {
                    annotation.start.saturating_sub(len_difference)
                } else {
                    annotation.start.saturating_add(len_difference)
                }
            } else if annotation.start >= start {
                // For annotations starting within the replaced range, we move the start index by the difference in length, constrained to the beginning or end of the replaced range.
                if shortened {
                    max(start, annotation.start.saturating_sub(len_difference))
                } else {
                    min(end, annotation.start.saturating_add(len_difference))
                }
            } else {
                annotation.start
            };

            annotation.end = if annotation.end >= end {
                // For annotations ending after the replaced range, we move the end index by the difference in length.
                if shortened {
                    annotation.end.saturating_sub(len_difference)
                } else {
                    annotation.end.saturating_add(len_difference)
                }
            } else if annotation.end >= start {
                // For annotations ending within the replaced range, we move the end index by the difference in length, constrained to the beginning or end of the replaced range.
                if shortened {
                    max(start, annotation.end.saturating_sub(len_difference))
                } else {
                    min(end, annotation.end.saturating_add(len_difference))
                }
            } else {
                annotation.end
            }
        });

        // Filter out empty annotations, in case the previous step resulted in any.
        self.annotations.retain(|annotation| {
            annotation.start < annotation.end && annotation.start < self.string.len()
        });
    }
}

impl Display for AnnotatedString {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.string)
    }
}

impl<'a> IntoIterator for &'a AnnotatedString {
    type Item = AnnotatedStringPart<'a>;
    type IntoIter = AnnotatedStringIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        AnnotatedStringIterator {
            annotated_string: self,
            current_index: 0,
        }
    }
}
