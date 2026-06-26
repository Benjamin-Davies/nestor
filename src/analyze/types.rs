use std::{borrow::Cow, fmt};

use tree_sitter::Node;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ident<'a> {
    pub bytes: &'a [u8],
    pub range: Range,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Range {
    pub start: Point,
    pub end: Point,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub row: u32,
    pub column: u32,
}

impl<'a> Ident<'a> {
    pub fn from_node(node: Node, source: &'a [u8]) -> Ident<'a> {
        debug_assert_eq!(node.kind(), "identifier");

        Ident {
            bytes: &source[node.byte_range()],
            range: node.range().into(),
        }
    }

    pub fn to_str(&self) -> Cow<'a, str> {
        String::from_utf8_lossy(self.bytes)
    }
}

impl From<tree_sitter::Range> for Range {
    fn from(value: tree_sitter::Range) -> Self {
        Self {
            start: value.start_point.into(),
            end: value.end_point.into(),
        }
    }
}

impl From<tree_sitter::Point> for Point {
    fn from(value: tree_sitter::Point) -> Self {
        Self {
            row: value.row as u32,
            column: value.column as u32,
        }
    }
}

impl From<Point> for tree_sitter::Point {
    fn from(value: Point) -> Self {
        Self {
            row: value.row as usize,
            column: value.column as usize,
        }
    }
}

impl From<Range> for lsp_types::Range {
    fn from(value: Range) -> Self {
        lsp_types::Range {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

impl From<lsp_types::Position> for Point {
    fn from(value: lsp_types::Position) -> Self {
        Point {
            row: value.line,
            column: value.character,
        }
    }
}

impl From<Point> for lsp_types::Position {
    fn from(value: Point) -> Self {
        lsp_types::Position {
            line: value.row,
            character: value.column,
        }
    }
}

impl fmt::Debug for Ident<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} ({})", self.to_str(), self.range)
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

impl fmt::Debug for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.row + 1, self.column)
    }
}

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
