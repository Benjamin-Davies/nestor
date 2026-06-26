use std::collections::BTreeMap;

use bytes::Bytes;

use crate::analyze::types::Range;

pub struct Locals {
    pub symbols: BTreeMap<Bytes, Vec<Range>>,
}

pub fn analyze(node: tree_sitter::Node, source: &Bytes) -> Locals {
    let mut symbols = BTreeMap::new();
    collect_symbols(node, source, &mut symbols);

    Locals { symbols }
}

fn collect_symbols(
    root: tree_sitter::Node,
    source: &Bytes,
    symbols: &mut BTreeMap<Bytes, Vec<Range>>,
) {
    let mut node = root;

    loop {
        if node.kind() == "identifier" {
            let name = source.slice(node.byte_range());
            let range = node.range().into();
            symbols.entry(name).or_default().push(range);
        }

        // Descend into children first (pre-order), otherwise try the next
        // sibling, otherwise walk back up until we find an ancestor that has
        // a next sibling (or we've exhausted the whole subtree).
        if let Some(child) = node.child(0) {
            node = child;
        } else {
            loop {
                if node == root {
                    return;
                }
                if let Some(sibling) = node.next_sibling() {
                    node = sibling;
                    break;
                }
                match node.parent() {
                    Some(parent) => node = parent,
                    None => return,
                }
            }
        }
    }
}
