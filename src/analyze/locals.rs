use std::collections::BTreeMap;

use bytes::Bytes;
use bytes_str::BytesStr;
use itertools::Itertools;

use crate::{
    analyze::{
        types::{Ident, SymbolKind},
        walk::{Scope, walk_symbols},
    },
    text::{PositionEncoding, PositionRange},
};

#[derive(Debug, Default)]
pub struct Locals {
    pub symbols: BTreeMap<Bytes, Vec<PositionRange>>,
    pub definitions: BTreeMap<Bytes, Vec<Definition>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Definition {
    pub name: PositionRange,
    pub scope: PositionRange,
    pub kind: SymbolKind,
}

pub fn analyze(node: tree_sitter::Node, source: &BytesStr, encoding: PositionEncoding) -> Locals {
    let mut locals = Locals::default();
    collect_symbols(node, source, encoding, &mut locals);

    locals
}

impl Locals {
    pub fn definitions(&self, ident: Ident) -> Vec<&Definition> {
        let Some(defs) = self.definitions.get(&ident.bytes) else {
            return Vec::new();
        };

        defs.iter()
            .filter(|d| {
                tracing::info!("{d:?}");
                d.scope.contains(ident.range)
            })
            .collect_vec()
    }
}

fn collect_symbols(
    root: tree_sitter::Node,
    source: &BytesStr,
    encoding: PositionEncoding,
    locals: &mut Locals,
) {
    let root_range = PositionRange::from_ts(root.range(), source, encoding);

    for symbol in walk_symbols(root) {
        let name_range = PositionRange::from_ts(symbol.range, source, encoding);
        let name = source
            .slice(symbol.range.start_byte..symbol.range.end_byte)
            .into_bytes();

        if symbol.definition {
            let scope = if let Scope::Block(range) = symbol.scope {
                PositionRange::from_ts(range, source, encoding)
            } else {
                root_range
            };

            locals
                .definitions
                .entry(name.clone())
                .or_default()
                .push(Definition {
                    name: name_range,
                    scope,
                    kind: symbol.kind,
                });
        }

        locals.symbols.entry(name).or_default().push(name_range);
    }
}
