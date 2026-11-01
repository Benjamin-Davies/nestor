use crate::analyze::{
    DECLARATION_KIND, ENUM_SPECIFIER_KIND, ENUMERATOR_KIND, EQUALS_KIND, FIELD_DECLARATION_KIND,
    FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND, IDENTIFIER_KIND, LEFT_BRACKET_KIND,
    PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND, PREPROC_FUNCTION_DEF_KIND, PREPROC_PARAMS_KIND,
    RIGHT_BRACKET_KIND, STATIC_KEYWORD_KIND, STRUCT_SPECIFIER_KIND, TYPE_DEFINITION_KIND,
    TYPE_IDENTIFIER_KIND, UNION_SPECIFIER_KIND, types::SymbolKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol<'a> {
    pub node: tree_sitter::Node<'a>,
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

#[derive(Debug, Clone, Copy)]
enum State {
    Start,
    VarDef,
    VarDefArraySize,
    FnDef { new_scope: tree_sitter::Range },
    FnDefName { new_scope: tree_sitter::Range },
    Macro,
    FunctionMacro { new_scope: tree_sitter::Range },
    TypeDef,
}

/// Iterates over the descendants of a node and their depths in pre-order,
/// skipping the node itself.
fn walk_preorder<'a>(
    node: tree_sitter::Node<'a>,
) -> impl Iterator<Item = (tree_sitter::Node<'a>, usize)> {
    let mut cursor = node.walk();
    let mut depth = 0;

    std::iter::from_fn(move || {
        if cursor.goto_first_child() {
            depth += 1;
            return Some((cursor.node(), depth));
        }

        loop {
            if cursor.goto_next_sibling() {
                return Some((cursor.node(), depth));
            }

            if cursor.goto_parent() {
                depth -= 1;
            } else {
                return None;
            }
        }
    })
}

pub fn walk_symbols<'a>(node: tree_sitter::Node<'a>) -> impl Iterator<Item = Symbol<'a>> {
    let mut state = State::Start;
    let mut scope = Scope::Global;
    let mut stack = Vec::<(State, Scope, usize)>::new();

    walk_preorder(node).filter_map(move |(node, depth)| {
        let kind = node.kind_id();
        let range = node.range();

        while let Some(&(next_state, next_scope, next_depth)) = stack.last()
            && next_depth >= depth
        {
            state = next_state;
            scope = next_scope;
            stack.pop();
        }

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
                            node,
                            definition: true,
                            scope,
                            kind: SymbolKind::Variable,
                        };
                    }
                    State::FnDefName { new_scope } => {
                        symbol = Symbol {
                            node,
                            definition: true,
                            scope,
                            kind: SymbolKind::Function,
                        };

                        state = State::Start;
                        scope = Scope::Block(new_scope);
                    }
                    State::Macro => {
                        symbol = Symbol {
                            node,
                            definition: true,
                            scope,
                            kind: SymbolKind::Macro,
                        };
                    }
                    State::FunctionMacro { new_scope } => {
                        symbol = Symbol {
                            node,
                            definition: true,
                            scope,
                            kind: SymbolKind::FunctionMacro,
                        };

                        state = State::Start;
                        scope = Scope::Block(new_scope);
                    }
                    _ => {
                        symbol = Symbol {
                            node,
                            definition: false,
                            scope,
                            kind: SymbolKind::Unknown,
                        };
                    }
                }

                Some(symbol)
            }
            STATIC_KEYWORD_KIND => {
                if scope == Scope::Global {
                    scope = Scope::File;
                }

                None
            }
            LEFT_BRACKET_KIND => {
                match state {
                    State::VarDef => {
                        state = State::VarDefArraySize;
                    }
                    _ => {}
                }

                None
            }
            RIGHT_BRACKET_KIND => {
                match state {
                    State::VarDefArraySize => {
                        state = State::VarDef;
                    }
                    _ => {}
                }

                None
            }
            EQUALS_KIND => {
                match state {
                    State::VarDef => {
                        state = State::Start;
                    }
                    _ => {}
                }

                None
            }
            DECLARATION_KIND
            | PARAMETER_DECLARATION_KIND
            | FIELD_DECLARATION_KIND
            | ENUMERATOR_KIND => {
                stack.push((state, scope, depth));
                state = State::VarDef;

                None
            }
            FUNCTION_DEFINITION_KIND => {
                stack.push((state, scope, depth));
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
                stack.push((state, scope, depth));
                state = State::Macro;

                None
            }
            PREPROC_FUNCTION_DEF_KIND => {
                stack.push((state, scope, depth));
                state = State::FunctionMacro { new_scope: range };

                None
            }
            PREPROC_PARAMS_KIND => {
                state = State::Start;

                None
            }
            TYPE_IDENTIFIER_KIND => {
                let symbol;
                match state {
                    State::TypeDef => {
                        symbol = Symbol {
                            node,
                            definition: true,
                            scope,
                            kind: SymbolKind::Type,
                        };
                    }
                    _ => {
                        symbol = Symbol {
                            node,
                            definition: false,
                            scope,
                            kind: SymbolKind::Type,
                        };
                    }
                }

                Some(symbol)
            }
            TYPE_DEFINITION_KIND
            | ENUM_SPECIFIER_KIND
            | STRUCT_SPECIFIER_KIND
            | UNION_SPECIFIER_KIND => {
                stack.push((state, scope, depth));
                state = State::TypeDef;

                None
            }
            _ => None,
        }
    })
}

fn range_contains(lhs: &tree_sitter::Range, rhs: &tree_sitter::Range) -> bool {
    lhs.start_byte <= rhs.start_byte && rhs.end_byte <= lhs.end_byte
}
