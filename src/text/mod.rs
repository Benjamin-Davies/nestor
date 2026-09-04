use std::{fmt, str::FromStr};

use anyhow::Context;

pub mod buffer;
pub mod flat_rope;
pub mod indices;
pub mod ropes;

/// The encoding that we use to encode positions. We always store text
/// as UTF-8 internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PositionEncoding {
    Utf8,
    Utf16,
}

/// Encapsulates an `lsp_types::Position` so that it is clear that these
/// encode the same semantics with respect to position encoding. We
/// store positions in their final format so that we can discard most of
/// the source text once we are done indexing a file.
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position(pub lsp_types::Position);

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PositionRange {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone)]
pub enum Change {
    Complete(String),
    Incremental(IncrementalChange),
}

#[derive(Debug, Clone)]
pub struct IncrementalChange {
    pub old_range: PositionRange,
    pub new_text: String,
}

impl PositionRange {
    pub fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    pub fn contains_pos(&self, pos: Position) -> bool {
        self.start <= pos && pos <= self.end
    }
}

impl From<PositionEncoding> for lsp_types::PositionEncodingKind {
    fn from(value: PositionEncoding) -> Self {
        match value {
            PositionEncoding::Utf8 => Self::UTF8,
            PositionEncoding::Utf16 => Self::UTF16,
        }
    }
}

impl From<lsp_types::Position> for Position {
    fn from(value: lsp_types::Position) -> Self {
        Self(value)
    }
}

impl From<Position> for lsp_types::Position {
    fn from(value: Position) -> Self {
        value.0
    }
}

impl From<lsp_types::Range> for PositionRange {
    fn from(value: lsp_types::Range) -> Self {
        PositionRange {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

impl From<PositionRange> for lsp_types::Range {
    fn from(value: PositionRange) -> Self {
        lsp_types::Range {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

impl From<lsp_types::TextDocumentContentChangeEvent> for Change {
    fn from(value: lsp_types::TextDocumentContentChangeEvent) -> Self {
        if let Some(old_range) = value.range {
            Self::Incremental(IncrementalChange {
                old_range: old_range.into(),
                new_text: value.text,
            })
        } else {
            Self::Complete(value.text)
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.0.line + 1, self.0.character)
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for Position {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let (row, character) = s.split_once(':').context("Expected ':' in point")?;
        Ok(Position(lsp_types::Position {
            line: row.parse::<u32>()?.saturating_sub(1),
            character: character.parse::<u32>()?,
        }))
    }
}

impl fmt::Display for PositionRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

impl fmt::Debug for PositionRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for PositionRange {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        if let Some((start, end)) = s.split_once('-') {
            let start = start.parse()?;
            let end = end.parse()?;
            Ok(PositionRange { start, end })
        } else {
            let position = s.parse()?;
            Ok(PositionRange {
                start: position,
                end: position,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::text::PositionRange;

    #[test]
    fn range_contains() {
        let a = "1:0-5:0".parse::<PositionRange>().unwrap();
        let b = "3:0-7:0".parse::<PositionRange>().unwrap();
        let c = "1:0-10:0".parse::<PositionRange>().unwrap();

        assert!(a.contains(a));
        assert!(!a.contains(b));
        assert!(!a.contains(c));
        assert!(!b.contains(a));
        assert!(b.contains(b));
        assert!(!b.contains(c));
        assert!(c.contains(a));
        assert!(c.contains(b));
        assert!(c.contains(c));
    }
}
