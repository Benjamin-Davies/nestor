use std::{
    cell::{OnceCell, RefCell},
    sync::OnceLock,
};

use anyhow::Context;
use tree_sitter::{Language, Parser, Tree};

pub mod dirs;
pub mod globals;
pub mod locals;
pub mod types;

mod walk;

pub fn language() -> &'static Language {
    static LANGUAGE: OnceLock<Language> = OnceLock::new();
    LANGUAGE.get_or_init(|| tree_sitter_c::LANGUAGE.into())
}

pub fn parse(source: &[u8], old_tree: Option<&Tree>) -> anyhow::Result<Tree> {
    with_parser(|parser| {
        parser
            .parse(source, old_tree)
            .context("error parsing source")
    })
}

fn with_parser<T>(f: impl FnOnce(&mut Parser) -> T) -> T {
    thread_local! {
        static PARSER: OnceCell<RefCell<Parser>> = OnceCell::new();
    }

    PARSER.with(|once_cell| {
        let mut parser = once_cell
            .get_or_init(|| {
                let mut parser = Parser::new();
                parser
                    .set_language(language())
                    .expect("error setting parser language");
                RefCell::new(parser)
            })
            .borrow_mut();

        f(&mut parser)
    })
}

pub(crate) const IDENTIFIER_KIND: u16 = 1;
pub(crate) const STATIC_KEYWORD_KIND: u16 = 71;
pub(crate) const LEFT_BRACKET_KIND: u16 = 70;
pub(crate) const RIGHT_BRACKET_KIND: u16 = 72;
pub(crate) const EQUALS_KIND: u16 = 73;
pub(crate) const PREPROC_DEF_KIND: u16 = 165;
pub(crate) const PREPROC_FUNCTION_DEF_KIND: u16 = 166;
pub(crate) const PREPROC_PARAMS_KIND: u16 = 167;
pub(crate) const FUNCTION_DEFINITION_KIND: u16 = 196;
pub(crate) const DECLARATION_KIND: u16 = 198;
pub(crate) const TYPE_DEFINITION_KIND: u16 = 199;
pub(crate) const FUNCTION_DECLARATOR_KIND: u16 = 230;
pub(crate) const ENUM_SPECIFIER_KIND: u16 = 247;
pub(crate) const STRUCT_SPECIFIER_KIND: u16 = 249;
pub(crate) const UNION_SPECIFIER_KIND: u16 = 250;
pub(crate) const FIELD_DECLARATION_KIND: u16 = 253;
pub(crate) const ENUMERATOR_KIND: u16 = 256;
pub(crate) const PARAMETER_DECLARATION_KIND: u16 = 260;
pub(crate) const TYPE_IDENTIFIER_KIND: u16 = 362;

pub const KEYWORDS: &[&str] = &[
    "alignas",
    "alignof",
    "auto",
    "bool",
    "break",
    "case",
    "char",
    "const",
    "constexpr",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "false",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "nullptr",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "struct",
    "switch",
    "thread_local",
    "true",
    "typedef",
    "typeof",
    "typeof_unqual",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
];

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use crate::analyze::{
        DECLARATION_KIND, ENUM_SPECIFIER_KIND, ENUMERATOR_KIND, EQUALS_KIND,
        FIELD_DECLARATION_KIND, FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND,
        IDENTIFIER_KIND, LEFT_BRACKET_KIND, PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND,
        PREPROC_FUNCTION_DEF_KIND, PREPROC_PARAMS_KIND, RIGHT_BRACKET_KIND, STATIC_KEYWORD_KIND,
        STRUCT_SPECIFIER_KIND, TYPE_DEFINITION_KIND, TYPE_IDENTIFIER_KIND, UNION_SPECIFIER_KIND,
        language, parse,
    };

    #[test]
    fn kind_ids() {
        let l = language();
        for (kind_id, kind, named) in [
            (IDENTIFIER_KIND, "identifier", true),
            (STATIC_KEYWORD_KIND, "static", false),
            (LEFT_BRACKET_KIND, "[", false),
            (RIGHT_BRACKET_KIND, "]", false),
            (EQUALS_KIND, "=", false),
            (PREPROC_DEF_KIND, "preproc_def", true),
            (PREPROC_FUNCTION_DEF_KIND, "preproc_function_def", true),
            (PREPROC_PARAMS_KIND, "preproc_params", true),
            (FUNCTION_DEFINITION_KIND, "function_definition", true),
            (DECLARATION_KIND, "declaration", true),
            (TYPE_DEFINITION_KIND, "type_definition", true),
            (FUNCTION_DECLARATOR_KIND, "function_declarator", true),
            (ENUM_SPECIFIER_KIND, "enum_specifier", true),
            (STRUCT_SPECIFIER_KIND, "struct_specifier", true),
            (UNION_SPECIFIER_KIND, "union_specifier", true),
            (FIELD_DECLARATION_KIND, "field_declaration", true),
            (ENUMERATOR_KIND, "enumerator", true),
            (PARAMETER_DECLARATION_KIND, "parameter_declaration", true),
            (TYPE_IDENTIFIER_KIND, "type_identifier", true),
        ] {
            let actual_kind_id = l.id_for_node_kind(kind, named);
            assert_eq!(
                kind_id, actual_kind_id,
                "node id for {kind:?} is {actual_kind_id} (not {kind_id})"
            );
        }
    }

    const BTREE_C: &str = include_str!("../../tests/btree.c");

    #[test]
    fn test_parse() {
        let source = Bytes::from(BTREE_C);
        let tree = parse(&source, None).unwrap();

        assert_eq!(tree.root_node().child_count(), 409);
    }

    #[test]
    fn test_parse_incremental() {
        let truncated_source = Bytes::from(&BTREE_C[..1000]);
        let old_tree = parse(&truncated_source, None).unwrap();

        let source = Bytes::from(BTREE_C);
        let tree = parse(&source, Some(&old_tree)).unwrap();

        assert_eq!(tree.root_node().child_count(), 409);
    }
}
