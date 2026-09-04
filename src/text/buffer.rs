use std::{
    cmp::Ordering,
    ops::{Deref, Range},
};

use bytes::Bytes;
use bytes_str::BytesStr;
use itertools::Itertools;

// Don't derive clone as consumers should clone the `BytesStr` instead.
#[derive(Debug, Default)]
pub struct TextBuffer {
    pub(super) inner: BytesStr,
    pub(super) line_cache: LineCache,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct LineCache {
    newline_indices: Vec<usize>,
    pub(super) buf_len: usize,
}

impl From<String> for TextBuffer {
    fn from(s: String) -> Self {
        let inner = BytesStr::from(s);
        let line_cache = LineCache::new(&inner);
        Self { inner, line_cache }
    }
}

impl TextBuffer {
    pub fn to_bytes(&self) -> Bytes {
        self.inner.clone().into_bytes()
    }

    pub fn to_bytes_str(&self) -> BytesStr {
        self.inner.clone()
    }

    pub fn slice(&self, byte_range: Range<usize>) -> BytesStr {
        self.inner.slice(byte_range)
    }
}

impl LineCache {
    pub(super) fn new(s: &str) -> Self {
        let newline_indices = s
            .bytes()
            .enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(|(i, _)| i)
            .collect_vec();

        Self {
            newline_indices,
            buf_len: s.len(),
        }
    }

    pub fn line(&self, line_index: usize) -> Option<Range<usize>> {
        match line_index.cmp(&self.newline_indices.len()) {
            Ordering::Greater => None,
            Ordering::Equal => {
                if let Some(&last_newline_index) = self.newline_indices.last() {
                    let start_index = last_newline_index + 1;
                    Some(start_index..self.buf_len)
                } else {
                    Some(0..self.buf_len)
                }
            }
            Ordering::Less => {
                let end_index = self.newline_indices[line_index];
                if line_index == 0 {
                    Some(0..end_index)
                } else {
                    let start_index = self.newline_indices[line_index - 1] + 1;
                    Some(start_index..end_index)
                }
            }
        }
    }

    pub fn byte_to_ts_point(&self, byte_index: usize) -> tree_sitter::Point {
        let Some(&last_newline_index) = self.newline_indices.last() else {
            return tree_sitter::Point {
                row: 0,
                column: byte_index,
            };
        };
        if byte_index > last_newline_index {
            return tree_sitter::Point {
                row: self.newline_indices.len(),
                column: byte_index - last_newline_index - 1,
            };
        }

        let next_newline = self
            .newline_indices
            .binary_search(&byte_index)
            .unwrap_or_else(|i| i);
        if next_newline == 0 {
            return tree_sitter::Point {
                row: 0,
                column: byte_index,
            };
        }

        let prev_newline_index = self.newline_indices[next_newline - 1];
        tree_sitter::Point {
            row: next_newline,
            column: byte_index - prev_newline_index - 1,
        }
    }

    pub(super) fn edit(&mut self, old_range: Range<usize>, new_text: &str) {
        let old_len = old_range.end - old_range.start;
        let new_len = new_text.len();

        let start_index = self
            .newline_indices
            .binary_search(&old_range.start)
            .unwrap_or_else(|i| i);
        let old_end_index = self
            .newline_indices
            .binary_search(&old_range.end)
            .unwrap_or_else(|i| i);

        for newline_index in &mut self.newline_indices[old_end_index..] {
            *newline_index -= old_len;
            *newline_index += new_len;
        }

        let new_indices = new_text
            .bytes()
            .enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(|(i, _)| i + old_range.start);
        self.newline_indices
            .splice(start_index..old_end_index, new_indices);

        self.buf_len -= old_len;
        self.buf_len += new_len;
    }
}

impl Deref for TextBuffer {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
