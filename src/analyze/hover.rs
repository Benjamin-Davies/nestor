use crate::{
    analyze::{
        ARRAY_DECLARATOR_KIND, DECLARATION_KIND, ENUM_SPECIFIER_KIND, ENUMERATOR_KIND,
        FIELD_DECLARATION_KIND, FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND,
        PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND, PREPROC_FUNCTION_DEF_KIND,
        STRUCT_SPECIFIER_KIND, TYPE_DEFINITION_KIND, UNION_SPECIFIER_KIND,
    },
    text::markdown::Markdown,
    text::{PositionEncoding, PositionRange, buffer::TextBuffer},
};

pub fn hover(
    source: &TextBuffer,
    encoding: PositionEncoding,
    tree: &tree_sitter::Tree,
    definition: PositionRange,
) -> Markdown {
    let ident = definition.to_ts(source, encoding);

    let mut snippet = extract_snippet(source, tree, ident);
    if !snippet.starts_with("#") {
        snippet += ";\n";
    }

    let mut md = Markdown::default();
    md.push_code_block(snippet);

    md
}

fn extract_snippet(
    source: &TextBuffer,
    tree: &tree_sitter::Tree,
    ident: tree_sitter::Range,
) -> String {
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
                return source[node.byte_range()].to_owned();
            }
            FUNCTION_DECLARATOR_KIND | ARRAY_DECLARATOR_KIND => {
                end_byte = node.end_byte();
            }
            FUNCTION_DEFINITION_KIND
            | DECLARATION_KIND
            | TYPE_DEFINITION_KIND
            | FIELD_DECLARATION_KIND => {
                return source[node.start_byte()..end_byte].to_owned();
            }
            _ => {}
        }
    }

    source[ident.start_byte..ident.end_byte].to_owned()
}
