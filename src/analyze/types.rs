use std::{borrow::Cow, fmt};

use bytes::Bytes;
use tree_sitter::Node;

use crate::{
    analyze::{IDENTIFIER_KIND, TYPE_IDENTIFIER_KIND},
    text::{PositionEncoding, PositionRange},
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ident {
    // Use bytes here as some files may not be UTF-8.
    pub bytes: Bytes,
    pub range: PositionRange,
    pub kind: SymbolKind,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolKind {
    #[default]
    Unknown,
    Variable,
    Function,
    Type,
    Macro,
    FunctionMacro,
    /// These are never actually stored, just used for completions.
    Keyword,
}

impl Ident {
    pub fn from_node(node: Node, source: &Bytes, encoding: PositionEncoding) -> Self {
        let bytes = source.slice(node.byte_range());
        let ts_kind = node.kind_id();
        debug_assert!(matches!(ts_kind, IDENTIFIER_KIND | TYPE_IDENTIFIER_KIND));
        Self {
            bytes,
            range: PositionRange::from_ts_bytes(node.range(), source, encoding),
            kind: if ts_kind == TYPE_IDENTIFIER_KIND {
                SymbolKind::Type
            } else {
                SymbolKind::Unknown
            },
        }
    }

    pub fn with_kind(self, kind: SymbolKind) -> Self {
        Self { kind, ..self }
    }

    pub fn to_str(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Unknown => "??",
            SymbolKind::Variable => "variable",
            SymbolKind::Function => "function",
            SymbolKind::Type => "type",
            SymbolKind::Macro => "macro",
            SymbolKind::FunctionMacro => "function macro",
            SymbolKind::Keyword => "keyword",
        }
    }
}

impl From<SymbolKind> for Option<lsp_types::CompletionItemKind> {
    fn from(value: SymbolKind) -> Self {
        use lsp_types::CompletionItemKind as K;
        match value {
            SymbolKind::Unknown => None,
            SymbolKind::Variable => Some(K::VARIABLE),
            SymbolKind::Function => Some(K::FUNCTION),
            SymbolKind::Type => Some(K::CLASS),
            SymbolKind::Macro => Some(K::CONSTANT),
            SymbolKind::FunctionMacro => Some(K::FUNCTION),
            SymbolKind::Keyword => Some(K::KEYWORD),
        }
    }
}

impl fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {} ({})", self.to_str(), self.kind, self.range)
    }
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
