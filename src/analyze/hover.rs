use std::ops::Range;

use itertools::Itertools;

use crate::{
    analyze::{
        ARRAY_DECLARATOR_KIND, COMMENT_KIND, DECLARATION_KIND, ENUM_SPECIFIER_KIND,
        ENUMERATOR_KIND, FIELD_DECLARATION_KIND, FUNCTION_DECLARATOR_KIND,
        FUNCTION_DEFINITION_KIND, PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND,
        PREPROC_FUNCTION_DEF_KIND, STRUCT_SPECIFIER_KIND, TYPE_DEFINITION_KIND,
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

fn find_comment(mut cursor: tree_sitter::TreeCursor) -> Option<Range<usize>> {
    if cursor.goto_previous_sibling() {
        let node = cursor.node();
        if node.kind_id() == COMMENT_KIND {
            return Some(node.byte_range());
        }
    }

    None
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
