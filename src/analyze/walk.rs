use crate::analyze::{
    DECLARATION_KIND, FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND, IDENTIFIER_KIND,
    PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND, PREPROC_FUNCTION_DEF_KIND, STATIC_KEYWORD_KIND,
    types::SymbolKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol {
    pub range: tree_sitter::Range,
    pub definition: bool,
    pub scope: Scope,
    pub kind: SymbolKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    File,
    Block(tree_sitter::Range),
}

enum State {
    Start,
    VarDef,
    FnDef { new_scope: tree_sitter::Range },
    FnDefName { new_scope: tree_sitter::Range },
    Macro,
    FunctionMacro,
}

/// Iterates over the descendants of a node in pre-order, skipping the node itself.
pub fn walk_preorder<'a>(
    node: tree_sitter::Node<'a>,
) -> impl Iterator<Item = tree_sitter::Node<'a>> {
    let mut cursor = node.walk();

    std::iter::from_fn(move || {
        if cursor.goto_first_child() {
            return Some(cursor.node());
        }

        loop {
            if cursor.goto_next_sibling() {
                return Some(cursor.node());
            }

            if !cursor.goto_parent() {
                return None;
            }
        }
    })
}

pub fn walk_symbols(node: tree_sitter::Node) -> impl Iterator<Item = Symbol> {
    let mut state = State::Start;
    let mut scope = Scope::Global;

    walk_preorder(node).filter_map(move |node| {
        let kind = node.kind_id();
        let range = node.range();

        if let Scope::Block(r) = &scope
            && !range_contains(r, &range)
        {
            scope = Scope::Global;
        }

        match kind {
            IDENTIFIER_KIND => {
                let symbol;
                match state {
                    State::VarDef => {
                        symbol = Symbol {
                            range,
                            definition: true,
                            scope,
                            kind: SymbolKind::Variable,
                        };

                        state = State::Start;
                    }
                    State::FnDefName { new_scope } => {
                        symbol = Symbol {
                            range,
                            definition: true,
                            scope,
                            kind: SymbolKind::Function,
                        };

                        state = State::Start;
                        scope = Scope::Block(new_scope);
                    }
                    State::Macro => {
                        symbol = Symbol {
                            range,
                            definition: true,
                            scope,
                            kind: SymbolKind::Macro,
                        };

                        state = State::Start;
                    }
                    State::FunctionMacro => {
                        symbol = Symbol {
                            range,
                            definition: true,
                            scope,
                            kind: SymbolKind::FunctionMacro,
                        };

                        state = State::Start;
                    }
                    _ => {
                        symbol = Symbol {
                            range,
                            definition: false,
                            scope,
                            kind: SymbolKind::Unknown,
                        };
                    }
                }

                if scope == Scope::File {
                    scope = Scope::Global;
                }

                Some(symbol)
            }
            STATIC_KEYWORD_KIND => {
                if scope == Scope::Global {
                    scope = Scope::File;
                }

                None
            }
            DECLARATION_KIND | PARAMETER_DECLARATION_KIND => {
                state = State::VarDef;

                None
            }
            FUNCTION_DEFINITION_KIND => {
                state = State::FnDef { new_scope: range };

                None
            }
            FUNCTION_DECLARATOR_KIND => {
                match state {
                    State::FnDef { new_scope } => {
                        state = State::FnDefName { new_scope };
                    }
                    _ => {}
                }

                None
            }
            PREPROC_DEF_KIND => {
                state = State::Macro;

                None
            }
            PREPROC_FUNCTION_DEF_KIND => {
                state = State::FunctionMacro;

                None
            }
            _ => None,
        }
    })
}

fn range_contains(lhs: &tree_sitter::Range, rhs: &tree_sitter::Range) -> bool {
    lhs.start_byte <= rhs.start_byte && rhs.end_byte <= lhs.end_byte
}
