use std::collections::BTreeMap;

use bytes::Bytes;
use bytes_str::BytesStr;
use itertools::Itertools;

use crate::{
    analyze::{
        DECLARATION_KIND, FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND, IDENTIFIER_KIND,
        PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND, PREPROC_FUNCTION_DEF_KIND,
        types::{Ident, SymbolKind},
        walk::walk_preorder,
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

enum State {
    Start,
    VarDef,
    FnDef { new_scope: PositionRange },
    FnDefName { new_scope: PositionRange },
    Macro,
    FunctionMacro,
}

fn collect_symbols(
    root: tree_sitter::Node,
    source: &BytesStr,
    encoding: PositionEncoding,
    locals: &mut Locals,
) {
    let root_range = PositionRange::from_ts(root.range(), source, encoding);

    let mut state = State::Start;
    let mut scope = root_range;
    for node in walk_preorder(root) {
        let kind = node.kind_id();
        let range = PositionRange::from_ts(node.range(), source, encoding);

        if !scope.contains(range) {
            scope = root_range;
        }

        match kind {
            IDENTIFIER_KIND => {
                let name = source.slice(node.byte_range()).into_bytes();
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
    }
}
