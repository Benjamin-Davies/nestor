use std::ops::Range;

use crate::text::{
    Position, PositionEncoding, PositionRange,
    buffer::{LineCache, TextBuffer},
};

impl Position {
    /// Unlike raw LSP positions, we trust tree-sitter points because we will have just parsed the document.
    fn from_ts(
        byte_index: usize,
        pos: tree_sitter::Point,
        s: &str,
        encoding: PositionEncoding,
    ) -> Self {
        match encoding {
            PositionEncoding::Utf8 => Self::from_ts_utf8(pos),
            PositionEncoding::Utf16 => Self::from_ts_utf16(byte_index, pos, s),
        }
    }

    fn from_ts_bytes(
        byte_index: usize,
        pos: tree_sitter::Point,
        bytes: &[u8],
        encoding: PositionEncoding,
    ) -> Self {
        match encoding {
            PositionEncoding::Utf8 => Self::from_ts_utf8(pos),
            PositionEncoding::Utf16 => Self::from_ts_utf16_bytes(byte_index, pos, bytes),
        }
    }

    fn from_ts_utf8(pos: tree_sitter::Point) -> Self {
        Self(lsp_types::Position {
            line: pos.row as u32,
            character: pos.column as u32,
        })
    }

    fn from_ts_utf16(byte_index: usize, pos: tree_sitter::Point, s: &str) -> Position {
        let line = &s[line_around(byte_index, s.as_bytes())];
        let utf16_index = str_indices::utf16::from_byte_idx(line, pos.column);

        Self(lsp_types::Position {
            line: pos.row as u32,
            character: utf16_index as u32,
        })
    }

    fn from_ts_utf16_bytes(byte_index: usize, pos: tree_sitter::Point, bytes: &[u8]) -> Position {
        let line_bytes = &bytes[line_around(byte_index, bytes)];
        let Ok(line) = str::from_utf8(line_bytes) else {
            // Give up and fall back to UTF-8 positions.
            return Self::from_ts_utf8(pos);
        };

        let utf16_index = str_indices::utf16::from_byte_idx(line, pos.column);

        Self(lsp_types::Position {
            line: pos.row as u32,
            character: utf16_index as u32,
        })
    }

    pub fn to_ts(
        self,
        buf: &TextBuffer,
        encoding: PositionEncoding,
    ) -> (usize, tree_sitter::Point) {
        self.to_ts_line_cache(buf, &buf.line_cache, encoding)
    }

    pub(super) fn to_ts_line_cache(
        self,
        buf: &str,
        cache: &LineCache,
        encoding: PositionEncoding,
    ) -> (usize, tree_sitter::Point) {
        let line_range = cache
            .line(self.0.line as usize)
            .unwrap_or(buf.len()..buf.len());
        let line = &buf[line_range.clone()];

        let index_in_line = match encoding {
            PositionEncoding::Utf8 => self.0.character as usize,
            PositionEncoding::Utf16 => {
                str_indices::utf16::to_byte_idx(line, self.0.character as usize)
            }
        };

        (
            line_range.start + index_in_line,
            tree_sitter::Point {
                row: self.0.line as usize,
                column: index_in_line,
            },
        )
    }
}

/// Returns the line around a given byte index, excluding the '\n's.
fn line_around(byte_index: usize, bytes: &[u8]) -> Range<usize> {
    let start = bytes[..byte_index]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = bytes[byte_index..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|i| i + byte_index)
        .unwrap_or(bytes.len());
    start..end
}

impl PositionRange {
    pub fn from_ts(range: tree_sitter::Range, s: &str, encoding: PositionEncoding) -> Self {
        PositionRange {
            start: Position::from_ts(range.start_byte, range.start_point, s, encoding),
            end: Position::from_ts(range.end_byte, range.end_point, s, encoding),
        }
    }

    pub fn from_ts_bytes(
        range: tree_sitter::Range,
        bytes: &[u8],
        encoding: PositionEncoding,
    ) -> Self {
        Self {
            start: Position::from_ts_bytes(range.start_byte, range.start_point, bytes, encoding),
            end: Position::from_ts_bytes(range.end_byte, range.end_point, bytes, encoding),
        }
    }
}
