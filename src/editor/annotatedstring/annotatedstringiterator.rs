use crate::prelude::*;
use std::cmp::min;

use super::{AnnotatedString, AnnotatedStringPart};

pub struct AnnotatedStringIterator<'a> {
    pub annotated_string: &'a AnnotatedString,
    pub current_index: ByteIndex,
}

impl<'a> Iterator for AnnotatedStringIterator<'a> {
    type Item = AnnotatedStringPart<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.annotated_string.string.len() {
            return None;
        }

        //Find the current active annotation
        if let Some(annotation) = self
            .annotated_string
            .annotations
            .iter()
            .filter(|annotation| {
                annotation.start <= self.current_index && annotation.end > self.current_index
            })
            .last()
        {
            let end_index = min(annotation.end, self.annotated_string.string.len());
            let start_index = self.current_index;
            self.current_index = end_index;
            return Some(AnnotatedStringPart {
                string: &self.annotated_string.string[start_index..end_index],
                annotation_type: Some(annotation.annotation_type),
            });
        }

        // Find the boundary of the nearest annotation
        let mut end_index = self.annotated_string.string.len();

        for annotation in &self.annotated_string.annotations {
            if annotation.start > self.current_index && annotation.start < end_index {
                end_index = annotation.start;
            }
        }

        let start_index = self.current_index;
        self.current_index = end_index;

        Some(AnnotatedStringPart {
            string: &self.annotated_string.string[start_index..end_index],
            annotation_type: None,
        })
    }
}
