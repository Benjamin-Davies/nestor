use std::collections::BTreeMap;

use bytes::Bytes;
use itertools::Itertools;
use ropey::Rope;

use crate::analyze::{
    DECLARATION_KIND, FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND, IDENTIFIER_KIND,
    PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND, PREPROC_FUNCTION_DEF_KIND,
    types::{Ident, Range, SymbolKind},
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
    pub kind: SymbolKind,
}

pub fn analyze(node: tree_sitter::Node, source: &Rope) -> Locals {
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
    VarDef,
    FnDef { new_scope: Range },
    FnDefName { new_scope: Range },
    Macro,
    FunctionMacro,
}

fn collect_symbols(root: tree_sitter::Node, source: &Rope, locals: &mut Locals) {
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
                // TODO: Be smarter about allocations
                let name = Bytes::from(source.slice(node.byte_range()).to_string());
                locals.symbols.entry(name.clone()).or_default().push(range);

                match state {
                    State::VarDef => {
                        locals
                            .definitions
                            .entry(name)
                            .or_default()
                            .push(Definition {
                                name: range,
                                scope,
                                kind: SymbolKind::Variable,
                            });

                        state = State::Start;
                    }
                    State::FnDefName { new_scope } => {
                        locals
                            .definitions
                            .entry(name)
                            .or_default()
                            .push(Definition {
                                name: range,
                                scope,
                                kind: SymbolKind::Function,
                            });

                        state = State::Start;
                        scope = new_scope;
                    }
                    State::Macro => {
                        locals
                            .definitions
                            .entry(name)
                            .or_default()
                            .push(Definition {
                                name: range,
                                scope,
                                kind: SymbolKind::Macro,
                            });

                        state = State::Start;
                    }
                    State::FunctionMacro => {
                        locals
                            .definitions
                            .entry(name)
                            .or_default()
                            .push(Definition {
                                name: range,
                                scope,
                                kind: SymbolKind::FunctionMacro,
                            });

                        state = State::Start;
                    }
                    _ => {}
                }
            }
            DECLARATION_KIND | PARAMETER_DECLARATION_KIND => {
                state = State::VarDef;
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
            PREPROC_DEF_KIND => {
                state = State::Macro;
            }
            PREPROC_FUNCTION_DEF_KIND => {
                state = State::FunctionMacro;
            }
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
