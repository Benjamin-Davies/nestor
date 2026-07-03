use std::collections::BTreeMap;

use bytes::Bytes;
use itertools::Itertools;

use crate::analyze::{
    FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND, IDENTIFIER_KIND,
    locals::State::Start,
    types::{Ident, Range},
};

#[derive(Debug, Default)]
pub struct Locals {
    pub symbols: BTreeMap<Bytes, Vec<Range>>,
    pub definitions: BTreeMap<Bytes, Vec<Definition>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Definition {
    pub name: Range,
    pub scope: Range,
}

pub fn analyze(node: tree_sitter::Node, source: &Bytes) -> Locals {
    let mut locals = Locals::default();
    collect_symbols(node, source, &mut locals);

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

enum State {
    Start,
    FnDef { new_scope: Range },
    FnDefName { new_scope: Range },
}

fn collect_symbols(root: tree_sitter::Node, source: &Bytes, locals: &mut Locals) {
    let root_range = Range::from(root.range());

    let mut node = root;
    let mut state = State::Start;
    let mut scope = root_range;
    loop {
        let kind = node.kind_id();
        let range = node.range().into();

        if !scope.contains(range) {
            scope = root_range;
        }

        match kind {
            IDENTIFIER_KIND => {
                let name = source.slice(node.byte_range());
                locals.symbols.entry(name.clone()).or_default().push(range);

                match state {
                    State::FnDefName { new_scope } => {
                        locals
                            .definitions
                            .entry(name)
                            .or_default()
                            .push(Definition { name: range, scope });

                        state = Start;
                        scope = new_scope;
                    }
                    _ => {}
                }
            }
            FUNCTION_DEFINITION_KIND => {
                state = State::FnDef { new_scope: range };
            }
            FUNCTION_DECLARATOR_KIND => match state {
                State::FnDef { new_scope } => {
                    state = State::FnDefName { new_scope };
                }
                _ => {}
            },
            _ => {}
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
