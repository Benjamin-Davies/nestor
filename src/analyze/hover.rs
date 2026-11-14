use std::ops::Range;

use itertools::Itertools;

use crate::{
    analyze::{
        ARRAY_DECLARATOR_KIND, COMMA_KIND, COMMENT_KIND, DECLARATION_KIND, ENUM_SPECIFIER_KIND,
        ENUMERATOR_KIND, FIELD_DECLARATION_KIND, FUNCTION_DECLARATOR_KIND,
        FUNCTION_DEFINITION_KIND, PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND,
        PREPROC_FUNCTION_DEF_KIND, SEMICOLON_KIND, STRUCT_SPECIFIER_KIND, TYPE_DEFINITION_KIND,
        UNION_SPECIFIER_KIND,
    },
    text::{PositionEncoding, PositionRange, buffer::TextBuffer, markdown::Markdown},
};

pub fn hover(
    source: &TextBuffer,
    encoding: PositionEncoding,
    tree: &tree_sitter::Tree,
    definition: PositionRange,
) -> Markdown {
    let ident = definition.to_ts(source, encoding);

    let (snippet_range, cursor) = find_snippet(tree, ident);
    let comment_range = find_comment(cursor);

    let mut md = Markdown::default();

    let mut snippet = source[snippet_range].to_owned();
    if !snippet.starts_with("#") {
        snippet += ";\n";
    }
    md.push_code_block(snippet);

    if let Some(range) = comment_range {
        let comment = strip_comment(&source[range]);
        md.push_raw(&comment);
    }

    md
}

fn find_snippet(
    tree: &tree_sitter::Tree,
    ident: tree_sitter::Range,
) -> (Range<usize>, tree_sitter::TreeCursor<'_>) {
    let mut cursor = tree.root_node().walk();
    while cursor.goto_first_child_for_byte(ident.start_byte).is_some() {}
    let mut end_byte = cursor.node().end_byte();
    while cursor.goto_parent() {
        let node = cursor.node();
        match node.kind_id() {
            PREPROC_DEF_KIND
            | PREPROC_FUNCTION_DEF_KIND
            | ENUM_SPECIFIER_KIND
            | STRUCT_SPECIFIER_KIND
            | UNION_SPECIFIER_KIND
            | ENUMERATOR_KIND
            | PARAMETER_DECLARATION_KIND => {
                return (node.byte_range(), cursor);
            }
            FUNCTION_DECLARATOR_KIND | ARRAY_DECLARATOR_KIND => {
                end_byte = node.end_byte();
            }
            FUNCTION_DEFINITION_KIND
            | DECLARATION_KIND
            | TYPE_DEFINITION_KIND
            | FIELD_DECLARATION_KIND => {
                return (node.start_byte()..end_byte, cursor);
            }
            _ => {}
        }
    }

    (ident.start_byte..ident.end_byte, cursor)
}

fn find_comment(cursor: tree_sitter::TreeCursor) -> Option<Range<usize>> {
    find_comment_before(cursor.clone()).or_else(|| find_comment_after(cursor))
}

fn find_comment_before(mut cursor: tree_sitter::TreeCursor) -> Option<Range<usize>> {
    if !cursor.goto_previous_sibling() {
        return None;
    }

    let node = cursor.node();
    if node.kind_id() != COMMENT_KIND {
        return None;
    }
    let end_byte = node.end_byte();

    let start_byte;
    let mut next_node = node;
    loop {
        if !cursor.goto_previous_sibling() {
            if next_node.kind_id() == COMMENT_KIND {
                start_byte = next_node.start_byte();
            } else {
                start_byte = next_node.end_byte();
            }
            break;
        }

        let node = cursor.node();
        if node.kind_id() != COMMENT_KIND {
            if !on_same_line(node, next_node) {
                start_byte = next_node.start_byte();
            } else {
                // next_node is an "after" comment
                start_byte = next_node.end_byte();
            }
            break;
        }

        next_node = node;
    }

    if start_byte < end_byte {
        Some(start_byte..end_byte)
    } else {
        None
    }
}

fn find_comment_after(mut cursor: tree_sitter::TreeCursor) -> Option<Range<usize>> {
    let definition = cursor.node();
    if !cursor.goto_next_sibling() {
        return None;
    }

    let mut node = cursor.node();
    if let COMMA_KIND | SEMICOLON_KIND = node.kind_id() {
        cursor.goto_next_sibling();
        node = cursor.node();
    }

    if node.kind_id() != COMMENT_KIND || !on_same_line(definition, node) {
        return None;
    }

    Some(node.byte_range())
}

fn on_same_line(a: tree_sitter::Node, b: tree_sitter::Node) -> bool {
    a.end_position().row == b.start_position().row
}

fn strip_comment(comment: &str) -> String {
    comment
        .trim_suffix("*/")
        .lines()
        .map(strip_comment_line)
        .join("\n")
}

fn strip_comment_line(line: &str) -> &str {
    line.trim()
        .trim_start_matches("/")
        .trim_start_matches("*")
        .trim_prefix(" ")
}
