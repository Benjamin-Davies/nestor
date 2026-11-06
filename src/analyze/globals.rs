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

#[derive(Debug, Default)]
pub struct Globals {
    pub symbols: Vec<Ident>,
    pub definitions: Vec<Ident>,
}

pub fn analyze<'a>(node: Node, source: Bytes, encoding: PositionEncoding) -> Globals {
    let mut globals = Globals::default();

    for symbol in walk_symbols(node) {
        let ident = Ident::from_node(symbol.node, &source, encoding).with_kind(symbol.kind);

        if symbol.definition && symbol.scope == Scope::Global {
            globals.definitions.push(ident.clone());
        }

        globals.symbols.push(ident);
    }

    globals
}
