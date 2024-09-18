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
        start_byte_idx: usize,
        end_byte_idx: usize,
    ) {
        debug_assert!(start_byte_idx <= end_byte_idx);
        self.annotations.push(Annotation {
            annotation_type,
            start_byte_index: start_byte_idx,
            end_byte_index: end_byte_idx,
        });
    }

    pub fn replace(&mut self, start_byte_index: usize, end_byte_index: usize, new_string: &str) {
        debug_assert!(start_byte_index <= end_byte_index);

        let end_byte_idx = min(end_byte_index, self.string.len());
        if start_byte_index > end_byte_idx {
            return;
        }
        self.string
            .replace_range(start_byte_index..end_byte_idx, new_string);

        let replaced_range_len = end_byte_idx.saturating_sub(start_byte_index); // This is the range we want to replace.
        let shortened = new_string.len() < replaced_range_len;
        let len_difference = new_string.len().abs_diff(replaced_range_len); // This is how much longer or shorter the new range is.

        if len_difference == 0 {
            //No adjustment of annotations needed in case the replacement did not result in a change in length.
            return;
        }

        self.annotations.iter_mut().for_each(|annotation| {
            annotation.start_byte_index = if annotation.start_byte_index >= end_byte_idx {
                // For annotations starting after the replaced range, we move the start index by the difference in length.
                if shortened {
                    annotation.start_byte_index.saturating_sub(len_difference)
                } else {
                    annotation.start_byte_index.saturating_add(len_difference)
                }
            } else if annotation.start_byte_index >= start_byte_index {
                // For annotations starting within the replaced range, we move the start index by the difference in length, constrained to the beginning or end of the replaced range.
                if shortened {
                    max(
                        start_byte_index,
                        annotation.start_byte_index.saturating_sub(len_difference),
                    )
                } else {
                    min(
                        end_byte_idx,
                        annotation.start_byte_index.saturating_add(len_difference),
                    )
                }
            } else {
                annotation.start_byte_index
            };

            annotation.end_byte_index = if annotation.end_byte_index >= end_byte_idx {
                // For annotations ending after the replaced range, we move the end index by the difference in length.
                if shortened {
                    annotation.end_byte_index.saturating_sub(len_difference)
                } else {
                    annotation.end_byte_index.saturating_add(len_difference)
                }
            } else if annotation.end_byte_index >= start_byte_index {
                // For annotations ending within the replaced range, we move the end index by the difference in length, constrained to the beginning or end of the replaced range.
                if shortened {
                    max(
                        start_byte_index,
                        annotation.end_byte_index.saturating_sub(len_difference),
                    )
                } else {
                    min(
                        end_byte_idx,
                        annotation.end_byte_index.saturating_add(len_difference),
                    )
                }
            } else {
                annotation.end_byte_index
            }
        });

        //Filter out empty annotations, in case the previous step resulted in any.
        self.annotations.retain(|annotation| {
            annotation.start_byte_index < annotation.end_byte_index
                && annotation.start_byte_index < self.string.len()
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
