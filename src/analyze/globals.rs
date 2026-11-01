//! Extract symbols that could be used from other files.

use bytes::Bytes;
use tree_sitter::Node;

use crate::{
    analyze::{
        types::Ident,
        walk::{Scope, walk_symbols},
    },
    text::PositionEncoding,
};

use super::types::SymbolKind;

#[derive(Debug, Default)]
pub struct Globals {
    pub symbols: Vec<Ident>,
    pub type_symbols: Vec<Ident>,
    pub definitions: Vec<Ident>,
    pub type_definitions: Vec<Ident>,
}

pub fn analyze<'a>(node: Node, source: Bytes, encoding: PositionEncoding) -> Globals {
    let mut globals = Globals::default();

    for symbol in walk_symbols(node) {
        let ident = Ident::from_node(symbol.node, &source, encoding).with_kind(symbol.kind);

        if symbol.definition && symbol.scope == Scope::Global {
            if symbol.kind == SymbolKind::Type {
                globals.type_definitions.push(ident.clone());
            } else {
                globals.definitions.push(ident.clone());
            }
        }

        if symbol.kind == SymbolKind::Type {
            globals.type_symbols.push(ident);
        } else {
            globals.symbols.push(ident);
        }
    }

    globals
}
