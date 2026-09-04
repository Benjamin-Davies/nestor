use std::fmt;

use anyhow::Context;
use bytes_str::{BytesStr, BytesString};
use smallvec::SmallVec;

use crate::text::{
    IncrementalChange, Position, PositionEncoding, PositionRange,
    buffer::{LineCache, TextBuffer},
};

/// Like a rope, but stored as a single flat array of chunks (plus a line cache).
pub struct FlatRope {
    segments: Vec<BytesStr>,
    line_cache: LineCache,
}

impl From<TextBuffer> for FlatRope {
    fn from(buf: TextBuffer) -> Self {
        Self {
            segments: vec![buf.inner],
            line_cache: buf.line_cache,
        }
    }
}

impl FlatRope {
    fn position_to_ts(
        &self,
        position: Position,
        encoding: PositionEncoding,
    ) -> Option<(usize, tree_sitter::Point)> {
        let row = position.0.line as usize;
        let line_range = self.line_cache.line(row)?;

        if position.0.character == 0 {
            return Some((
                line_range.start,
                tree_sitter::Point {
                    row: position.0.line as usize,
                    column: 0,
                },
            ));
        }

        // Find the first segment of the line
        let mut segments = self.segments.iter();
        let mut segment = segments.next()?.as_str();
        let mut start_offset = line_range.start;
        while start_offset >= segment.len() {
            start_offset -= segment.len();
            segment = segments.next()?.as_str();
        }
        segment = &segment[start_offset..];

        // Traverse into the line
        let mut column = 0;
        let mut remaining_code_units = position.0.character as usize;
        match encoding {
            PositionEncoding::Utf8 => {
                while remaining_code_units > segment.len() {
                    column += segment.len();
                    remaining_code_units -= segment.len();
                    segment = segments.next()?.as_str();
                }
                column += segment.floor_char_boundary(remaining_code_units);
            }
            PositionEncoding::Utf16 => {
                while remaining_code_units > 0 {
                    let byte_index = str_indices::utf16::to_byte_idx(segment, remaining_code_units);
                    if byte_index == segment.len() {
                        column += byte_index;
                        remaining_code_units -= str_indices::utf16::count(segment);
                        segment = segments.next()?.as_str();
                    } else {
                        column += byte_index;
                        break;
                    }
                }
            }
        }

        if column > line_range.end - line_range.start {
            return None;
        }

        Some((
            line_range.start + column,
            tree_sitter::Point { row, column },
        ))
    }

    fn range_to_ts(
        &self,
        range: PositionRange,
        encoding: PositionEncoding,
    ) -> anyhow::Result<tree_sitter::Range> {
        let (start_byte, start_point) = self
            .position_to_ts(range.start, encoding)
            .context("Invalid start of range")?;
        let (end_byte, end_point) = self
            .position_to_ts(range.end, encoding)
            .context("Invalid end of range")?;
        Ok(tree_sitter::Range {
            start_byte,
            end_byte,
            start_point,
            end_point,
        })
    }

    pub fn edit(
        &mut self,
        change: IncrementalChange,
        encoding: PositionEncoding,
    ) -> anyhow::Result<tree_sitter::InputEdit> {
        let tree_sitter::Range {
            start_byte,
            end_byte: old_end_byte,
            start_point: start_position,
            end_point: old_end_position,
        } = self.range_to_ts(change.old_range, encoding)?;
        let new_end_byte = start_byte + change.new_text.len();

        self.line_cache
            .edit(start_byte..old_end_byte, &change.new_text);

        self.edit_inner(start_byte, old_end_byte, change.new_text);

        let new_end_position = self.line_cache.byte_to_ts_point(new_end_byte);

        Ok(tree_sitter::InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        })
    }

    fn edit_inner(&mut self, start_byte: usize, old_end_byte: usize, new_text: String) {
        let (start_segment, start_offset) = self.segment_offset(start_byte);
        let (mut old_end_segment, old_end_offset) = self.segment_offset(old_end_byte);

        let mut new_segments = SmallVec::<[BytesStr; 3]>::new();

        if start_offset > 0 {
            let old_segment = &self.segments[start_segment];
            new_segments.push(old_segment.slice(..start_offset));
        }

        if !new_text.is_empty() {
            new_segments.push(new_text.into());
        }

        if old_end_offset > 0 {
            let old_segment = &self.segments[old_end_segment];
            new_segments.push(old_segment.slice(old_end_offset..));
            old_end_segment += 1;
        }

        self.segments
            .splice(start_segment..old_end_segment, new_segments);
    }

    fn segment_offset(&self, byte_index: usize) -> (usize, usize) {
        if self.segments.is_empty() {
            return (0, 0);
        }

        let mut segment = 0;
        let mut remaining_bytes = byte_index;
        while remaining_bytes > self.segments[segment].len() {
            remaining_bytes -= self.segments[segment].len();
            segment += 1;
        }

        (segment, remaining_bytes)
    }

    pub fn freeze(self) -> TextBuffer {
        let buf = match self.segments.len() {
            0 => BytesStr::new(),
            1 => self.segments.into_iter().next().unwrap(),
            _ => {
                let capacity = self.segments.iter().map(BytesStr::len).sum();
                let mut buf = BytesString::with_capacity(capacity);
                for segment in &self.segments {
                    buf.push_str(segment);
                }
                buf.into()
            }
        };

        TextBuffer {
            inner: buf,
            line_cache: self.line_cache,
        }
    }
}

impl fmt::Display for FlatRope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for s in &self.segments {
            f.write_str(s)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::text::{
        IncrementalChange, PositionEncoding,
        buffer::{LineCache, TextBuffer},
        flat_rope::FlatRope,
    };

    #[test]
    fn test_position_to_ts() {
        let text = "foo\nbar\n";
        let rope = FlatRope {
            segments: vec![
                text[0..2].into(),
                text[2..4].into(),
                text[4..6].into(),
                text[6..].into(),
            ],
            line_cache: LineCache::new(text),
        };

        assert_eq!(
            rope.position_to_ts("1:0".parse().unwrap(), PositionEncoding::Utf8),
            Some((0, tree_sitter::Point { row: 0, column: 0 }))
        );
        assert_eq!(
            rope.position_to_ts("1:1".parse().unwrap(), PositionEncoding::Utf8),
            Some((1, tree_sitter::Point { row: 0, column: 1 }))
        );
        assert_eq!(
            rope.position_to_ts("1:2".parse().unwrap(), PositionEncoding::Utf8),
            Some((2, tree_sitter::Point { row: 0, column: 2 }))
        );
        assert_eq!(
            rope.position_to_ts("1:3".parse().unwrap(), PositionEncoding::Utf8),
            Some((3, tree_sitter::Point { row: 0, column: 3 }))
        );
        assert_eq!(
            rope.position_to_ts("1:4".parse().unwrap(), PositionEncoding::Utf8),
            None
        );

        assert_eq!(
            rope.position_to_ts("2:0".parse().unwrap(), PositionEncoding::Utf8),
            Some((4, tree_sitter::Point { row: 1, column: 0 }))
        );
        assert_eq!(
            rope.position_to_ts("2:1".parse().unwrap(), PositionEncoding::Utf8),
            Some((5, tree_sitter::Point { row: 1, column: 1 }))
        );
        assert_eq!(
            rope.position_to_ts("2:2".parse().unwrap(), PositionEncoding::Utf8),
            Some((6, tree_sitter::Point { row: 1, column: 2 }))
        );
        assert_eq!(
            rope.position_to_ts("2:3".parse().unwrap(), PositionEncoding::Utf8),
            Some((7, tree_sitter::Point { row: 1, column: 3 }))
        );
        assert_eq!(
            rope.position_to_ts("2:4".parse().unwrap(), PositionEncoding::Utf8),
            None
        );

        assert_eq!(
            rope.position_to_ts("3:0".parse().unwrap(), PositionEncoding::Utf8),
            Some((8, tree_sitter::Point { row: 2, column: 0 }))
        );
        assert_eq!(
            rope.position_to_ts("3:1".parse().unwrap(), PositionEncoding::Utf8),
            None
        );
    }

    #[test]
    fn test_position_to_ts_unicode() {
        let text = "hello\n😎😎😎";
        let rope = FlatRope {
            segments: vec![
                text[0..2].into(),
                text[2..4].into(),
                text[4..10].into(),
                text[10..].into(),
            ],
            line_cache: LineCache::new(text),
        };

        assert_eq!(
            rope.position_to_ts("2:4".parse().unwrap(), PositionEncoding::Utf8),
            Some((10, tree_sitter::Point { row: 1, column: 4 }))
        );
        assert_eq!(
            rope.position_to_ts("2:8".parse().unwrap(), PositionEncoding::Utf8),
            Some((14, tree_sitter::Point { row: 1, column: 8 }))
        );
        assert_eq!(
            rope.position_to_ts("2:10".parse().unwrap(), PositionEncoding::Utf8),
            Some((14, tree_sitter::Point { row: 1, column: 8 }))
        );
    }

    #[test]
    fn test_position_to_ts_utf16() {
        let text = "foo\nbar\n";
        let rope = FlatRope {
            segments: vec![
                text[0..2].into(),
                text[2..4].into(),
                text[4..6].into(),
                text[6..].into(),
            ],
            line_cache: LineCache::new(text),
        };

        assert_eq!(
            rope.position_to_ts("1:0".parse().unwrap(), PositionEncoding::Utf16),
            Some((0, tree_sitter::Point { row: 0, column: 0 }))
        );
        assert_eq!(
            rope.position_to_ts("1:1".parse().unwrap(), PositionEncoding::Utf16),
            Some((1, tree_sitter::Point { row: 0, column: 1 }))
        );
        assert_eq!(
            rope.position_to_ts("1:2".parse().unwrap(), PositionEncoding::Utf16),
            Some((2, tree_sitter::Point { row: 0, column: 2 }))
        );
        assert_eq!(
            rope.position_to_ts("1:3".parse().unwrap(), PositionEncoding::Utf16),
            Some((3, tree_sitter::Point { row: 0, column: 3 }))
        );
        assert_eq!(
            rope.position_to_ts("1:4".parse().unwrap(), PositionEncoding::Utf16),
            None
        );

        assert_eq!(
            rope.position_to_ts("2:0".parse().unwrap(), PositionEncoding::Utf16),
            Some((4, tree_sitter::Point { row: 1, column: 0 }))
        );
        assert_eq!(
            rope.position_to_ts("2:1".parse().unwrap(), PositionEncoding::Utf16),
            Some((5, tree_sitter::Point { row: 1, column: 1 }))
        );
        assert_eq!(
            rope.position_to_ts("2:2".parse().unwrap(), PositionEncoding::Utf16),
            Some((6, tree_sitter::Point { row: 1, column: 2 }))
        );
        assert_eq!(
            rope.position_to_ts("2:3".parse().unwrap(), PositionEncoding::Utf16),
            Some((7, tree_sitter::Point { row: 1, column: 3 }))
        );
        assert_eq!(
            rope.position_to_ts("2:4".parse().unwrap(), PositionEncoding::Utf16),
            None
        );

        assert_eq!(
            rope.position_to_ts("3:0".parse().unwrap(), PositionEncoding::Utf16),
            Some((8, tree_sitter::Point { row: 2, column: 0 }))
        );
        assert_eq!(
            rope.position_to_ts("3:1".parse().unwrap(), PositionEncoding::Utf16),
            None
        );
    }

    #[test]
    fn test_position_to_ts_unicode_utf16() {
        let text = "hello\n😎😎😎";
        let rope = FlatRope {
            segments: vec![
                text[0..2].into(),
                text[2..4].into(),
                text[4..10].into(),
                text[10..].into(),
            ],
            line_cache: LineCache::new(text),
        };

        assert_eq!(
            rope.position_to_ts("2:2".parse().unwrap(), PositionEncoding::Utf16),
            Some((10, tree_sitter::Point { row: 1, column: 4 }))
        );
        assert_eq!(
            rope.position_to_ts("2:4".parse().unwrap(), PositionEncoding::Utf16),
            Some((14, tree_sitter::Point { row: 1, column: 8 }))
        );
        assert_eq!(
            rope.position_to_ts("2:5".parse().unwrap(), PositionEncoding::Utf16),
            Some((14, tree_sitter::Point { row: 1, column: 8 }))
        );
    }

    #[test]
    fn test_edit() {
        let text = TextBuffer::from("foo\nbar\nbaz\n".to_owned());
        let mut rope = FlatRope::from(text);

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "1:3-1:3".parse().unwrap(),
                    new_text: "d".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 3,
                old_end_byte: 3,
                new_end_byte: 4,
                start_position: tree_sitter::Point { row: 0, column: 3 },
                old_end_position: tree_sitter::Point { row: 0, column: 3 },
                new_end_position: tree_sitter::Point { row: 0, column: 4 }
            }
        );
        assert_eq!(rope.to_string(), "food\nbar\nbaz\n");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "1:2-2:2".parse().unwrap(),
                    new_text: "".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 2,
                old_end_byte: 7,
                new_end_byte: 2,
                start_position: tree_sitter::Point { row: 0, column: 2 },
                old_end_position: tree_sitter::Point { row: 1, column: 2 },
                new_end_position: tree_sitter::Point { row: 0, column: 2 }
            }
        );
        assert_eq!(rope.to_string(), "for\nbaz\n");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "2:1-2:3".parse().unwrap(),
                    new_text: "etter".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 5,
                old_end_byte: 7,
                new_end_byte: 10,
                start_position: tree_sitter::Point { row: 1, column: 1 },
                old_end_position: tree_sitter::Point { row: 1, column: 3 },
                new_end_position: tree_sitter::Point { row: 1, column: 6 }
            }
        );
        assert_eq!(rope.to_string(), "for\nbetter\n");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "2:6-3:0".parse().unwrap(),
                    new_text: " or worse".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 10,
                old_end_byte: 11,
                new_end_byte: 19,
                start_position: tree_sitter::Point { row: 1, column: 6 },
                old_end_position: tree_sitter::Point { row: 2, column: 0 },
                new_end_position: tree_sitter::Point { row: 1, column: 15 }
            }
        );
        assert_eq!(rope.to_string(), "for\nbetter or worse");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "1:3-2:0".parse().unwrap(),
                    new_text: " ".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 3,
                old_end_byte: 4,
                new_end_byte: 4,
                start_position: tree_sitter::Point { row: 0, column: 3 },
                old_end_position: tree_sitter::Point { row: 1, column: 0 },
                new_end_position: tree_sitter::Point { row: 0, column: 4 }
            }
        );
        assert_eq!(rope.to_string(), "for better or worse");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        assert_eq!(rope.freeze().to_string(), "for better or worse");
    }

    #[test]
    fn test_edit_all() {
        let text = TextBuffer::from("foo\nbar\nbaz\n".to_owned());
        let mut rope = FlatRope::from(text);

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "1:3-1:3".parse().unwrap(),
                    new_text: "d".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 3,
                old_end_byte: 3,
                new_end_byte: 4,
                start_position: tree_sitter::Point { row: 0, column: 3 },
                old_end_position: tree_sitter::Point { row: 0, column: 3 },
                new_end_position: tree_sitter::Point { row: 0, column: 4 }
            }
        );
        assert_eq!(rope.to_string(), "food\nbar\nbaz\n");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "1:0-4:0".parse().unwrap(),
                    new_text: "hello".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 0,
                old_end_byte: 13,
                new_end_byte: 5,
                start_position: tree_sitter::Point { row: 0, column: 0 },
                old_end_position: tree_sitter::Point { row: 3, column: 0 },
                new_end_position: tree_sitter::Point { row: 0, column: 5 }
            }
        );
        assert_eq!(rope.to_string(), "hello");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        let edit = rope
            .edit(
                IncrementalChange {
                    old_range: "1:5-1:5".parse().unwrap(),
                    new_text: ", world".to_owned(),
                },
                PositionEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(
            edit,
            tree_sitter::InputEdit {
                start_byte: 5,
                old_end_byte: 5,
                new_end_byte: 12,
                start_position: tree_sitter::Point { row: 0, column: 5 },
                old_end_position: tree_sitter::Point { row: 0, column: 5 },
                new_end_position: tree_sitter::Point { row: 0, column: 12 }
            }
        );
        assert_eq!(rope.to_string(), "hello, world");
        assert_eq!(rope.line_cache, LineCache::new(&rope.to_string()));

        assert_eq!(rope.freeze().to_string(), "hello, world");
    }
}
