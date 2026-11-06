use std::{borrow::Cow, fmt};

use bytes::Bytes;
use tree_sitter::Node;

use crate::{
    analyze::{FIELD_IDENTIFIER_KIND, TYPE_IDENTIFIER_KIND},
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
    Field,
    EnumVariant,
    Macro,
    FunctionMacro,
    /// These are never actually stored, just used for completions.
    Keyword,
}

/// Groups of SymbolKinds, distinguishable just by looking at the usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolClass {
    Type,
    Field,
    Other,
}

impl Ident {
    pub fn from_node(node: Node, source: &Bytes, encoding: PositionEncoding) -> Self {
        let bytes = source.slice(node.byte_range());
        let kind = SymbolClass::for_ident(node).into();
        Self {
            bytes,
            range: PositionRange::from_ts_bytes(node.range(), source, encoding),
            kind,
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
            SymbolKind::Field => "field",
            SymbolKind::EnumVariant => "enum variant",
            SymbolKind::Macro => "macro",
            SymbolKind::FunctionMacro => "function macro",
            SymbolKind::Keyword => "keyword",
        }
    }

    pub fn class(self) -> SymbolClass {
        match self {
            SymbolKind::Type => SymbolClass::Type,
            SymbolKind::Field => SymbolClass::Field,
            _ => SymbolClass::Other,
        }
    }
}

impl SymbolClass {
    pub fn for_ident(node: Node) -> Self {
        match node.kind_id() {
            TYPE_IDENTIFIER_KIND => SymbolClass::Type,
            FIELD_IDENTIFIER_KIND => SymbolClass::Field,
            _ => SymbolClass::Other,
        }
    }
}

impl From<SymbolClass> for SymbolKind {
    fn from(value: SymbolClass) -> Self {
        match value {
            SymbolClass::Type => SymbolKind::Type,
            SymbolClass::Field => SymbolKind::Field,
            SymbolClass::Other => SymbolKind::Unknown,
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
            SymbolKind::Field => Some(K::FIELD),
            SymbolKind::EnumVariant => Some(K::ENUM_MEMBER),
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
