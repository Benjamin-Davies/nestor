//! Extract symbols that could be used from other files.

use std::sync::OnceLock;

use bytes::Bytes;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::analyze::{IDENTIFIER_KIND, STORAGE_CLASS_SPECIFIER_KIND, language, types::Ident};

use super::types::SymbolKind;

#[derive(Debug, Default)]
pub struct Globals {
    pub symbols: Vec<Ident>,
    pub type_symbols: Vec<Ident>,
    pub definitions: Vec<Ident>,
    pub type_definitions: Vec<Ident>,
}

pub fn ident_query() -> &'static Query {
    const SOURCE: &str = "(identifier) @ident";

    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| Query::new(language(), SOURCE).expect("error parsing query"))
}

pub fn type_ident_query() -> &'static Query {
    const SOURCE: &str = "(type_identifier) @ident";

    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| Query::new(language(), SOURCE).expect("error parsing query"))
}

pub fn fn_def_query() -> &'static Query {
    const SOURCE: &str = "
            (function_definition
                (storage_class_specifier)? @storage_class
                declarator: (function_declarator
                    declarator: (identifier) @ident))
        ";

    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| Query::new(language(), SOURCE).expect("error parsing query"))
}

pub fn var_decl_query() -> &'static Query {
    const SOURCE: &str = "
        [
            (declaration
                (storage_class_specifier)? @storage_class
                declarator: (identifier) @ident)
            (declaration
                (storage_class_specifier)? @storage_class
                [
                    (pointer_declarator
                        declarator: (identifier) @ident)
                    (array_declarator
                        declarator: (identifier) @ident)
                    (init_declarator
                        declarator: (identifier) @ident)
                ])
        ]
    ";

    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| Query::new(language(), SOURCE).expect("error parsing query"))
}

pub fn type_def_query() -> &'static Query {
    const SOURCE: &str = "
        [
            (type_definition
                declarator: (type_identifier) @ident)
            (pointer_declarator
                declarator: (type_identifier) @ident)
            (array_declarator
                declarator: (type_identifier) @ident)
            (struct_specifier
                name: (type_identifier) @ident)
            (union_specifier
                name: (type_identifier) @ident)
            (enum_specifier
                name: (type_identifier) @ident)
        ]
    ";

    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| Query::new(language(), SOURCE).expect("error parsing query"))
}

pub fn macro_def_query() -> &'static Query {
    const SOURCE: &str = "
        [
            (preproc_def
                name: (identifier) @macro_name)
            (preproc_function_def
                name: (identifier) @function_macro_name)
        ]
    ";

    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| Query::new(language(), SOURCE).expect("error parsing query"))
}

pub fn analyze<'a>(node: Node, source: Bytes) -> Globals {
    let mut globals = Globals::default();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(ident_query(), node, &*source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let node = capture.node;
            if node.byte_range().is_empty() {
                continue;
            }

            let ident = Ident::from_node(node, &source);
            globals.symbols.push(ident);
        }
    }

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(type_ident_query(), node, &*source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let node = capture.node;
            if node.byte_range().is_empty() {
                continue;
            }

            let ident = Ident::from_node(node, &source).with_kind(SymbolKind::Type);
            globals.type_symbols.push(ident);
        }
    }

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(fn_def_query(), node, &*source);
    while let Some(m) = matches.next() {
        'captures: for capture in m.captures {
            let node = capture.node;
            match node.kind_id() {
                STORAGE_CLASS_SPECIFIER_KIND => {
                    let bytes = &source[node.byte_range()];
                    if bytes == b"static" {
                        break 'captures;
                    }
                }
                IDENTIFIER_KIND => {
                    if node.byte_range().is_empty() {
                        continue;
                    }

                    let ident = Ident::from_node(node, &source).with_kind(SymbolKind::Function);
                    globals.definitions.push(ident);
                }
                kind => unreachable!("{kind}"),
            }
        }
    }

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(var_decl_query(), node, &*source);
    while let Some(m) = matches.next() {
        'captures: for capture in m.captures {
            let node = capture.node;
            if is_inside_fn(node) {
                break 'captures;
            }

            match node.kind_id() {
                STORAGE_CLASS_SPECIFIER_KIND => {
                    let bytes = &source[node.byte_range()];
                    if bytes == b"static" {
                        break 'captures;
                    }
                }
                IDENTIFIER_KIND => {
                    if node.byte_range().is_empty() {
                        continue;
                    }

                    let ident = Ident::from_node(node, &source).with_kind(SymbolKind::Variable);
                    globals.definitions.push(ident);
                }
                kind => unreachable!("{kind}"),
            }
        }
    }

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(type_def_query(), node, &*source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let node = capture.node;
            if node.byte_range().is_empty() {
                continue;
            }

            let ident = Ident::from_node(node, &source).with_kind(SymbolKind::Type);
            globals.type_definitions.push(ident);
        }
    }

    let query = macro_def_query();
    let macro_name_index = query
        .capture_index_for_name("macro_name")
        .expect("unable to find capture index");
    let function_macro_name_index = query
        .capture_index_for_name("function_macro_name")
        .expect("unable to find capture index");
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, &*source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let node = capture.node;
            if node.byte_range().is_empty() {
                continue;
            }

            if capture.index == macro_name_index {
                let ident = Ident::from_node(node, &source).with_kind(SymbolKind::Macro);
                globals.definitions.push(ident);
            } else if capture.index == function_macro_name_index {
                let ident = Ident::from_node(node, &source).with_kind(SymbolKind::FunctionMacro);
                globals.definitions.push(ident);
            }
        }
    }

    globals
}

fn is_inside_fn(node: Node) -> bool {
    let mut node = node;
    loop {
        if node.kind() == "function_definition" {
            return true;
        }

        if let Some(parent) = node.parent() {
            node = parent;
        } else {
            return false;
        }
    }
}
