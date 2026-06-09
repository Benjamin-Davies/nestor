//! Extract symbols that could be used from other files.

use std::sync::OnceLock;

use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::analyze::{language, types::Ident};

#[derive(Debug, Default)]
pub struct Globals<'a> {
    pub symbols: Vec<Ident<'a>>,
    pub definitions: Vec<Ident<'a>>,
}

pub fn ident_query() -> &'static Query {
    const SOURCE: &str = "(identifier) @ident";

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

pub fn analyze<'a>(node: Node, source: &'a [u8]) -> Globals<'a> {
    let mut globals = Globals::default();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(ident_query(), node, source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let node = capture.node;
            if node.byte_range().is_empty() {
                continue;
            }

            let ident = Ident::from_node(node, source);
            globals.symbols.push(ident);
        }
    }

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(fn_def_query(), node, source);
    while let Some(m) = matches.next() {
        'captures: for capture in m.captures {
            let node = capture.node;
            match node.kind() {
                "storage_class_specifier" => {
                    let bytes = &source[node.byte_range()];
                    if bytes == b"static" {
                        break 'captures;
                    }
                }
                "identifier" => {
                    if node.byte_range().is_empty() {
                        continue;
                    }

                    let ident = Ident::from_node(node, source);
                    globals.definitions.push(ident);
                }
                kind => unreachable!("{kind}"),
            }
        }
    }

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(var_decl_query(), node, source);
    while let Some(m) = matches.next() {
        'captures: for capture in m.captures {
            let node = capture.node;
            if is_inside_fn(node) {
                break 'captures;
            }

            match node.kind() {
                "storage_class_specifier" => {
                    let bytes = &source[node.byte_range()];
                    if bytes == b"static" {
                        break 'captures;
                    }
                }
                "identifier" => {
                    if node.byte_range().is_empty() {
                        continue;
                    }

                    let ident = Ident::from_node(node, source);
                    globals.definitions.push(ident);
                }
                kind => unreachable!("{kind}"),
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
