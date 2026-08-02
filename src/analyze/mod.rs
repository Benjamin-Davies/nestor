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

pub fn language() -> &'static Language {
    static LANGUAGE: OnceLock<Language> = OnceLock::new();
    LANGUAGE.get_or_init(|| tree_sitter_c::LANGUAGE.into())
}

pub fn parse(source: &[u8]) -> anyhow::Result<Tree> {
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

        parser.parse(source, None).context("error parsing source")
    })
}

pub const IDENTIFIER_KIND: u16 = 1;
pub const PREPROC_DEF_KIND: u16 = 165;
pub const PREPROC_FUNCTION_DEF_KIND: u16 = 166;
pub const FUNCTION_DEFINITION_KIND: u16 = 196;
pub const DECLARATION_KIND: u16 = 198;
pub const FUNCTION_DECLARATOR_KIND: u16 = 230;
pub const STORAGE_CLASS_SPECIFIER_KIND: u16 = 242;
pub const PARAMETER_DECLARATION_KIND: u16 = 260;
pub const TYPE_IDENTIFIER_KIND: u16 = 362;

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
    use crate::analyze::{
        DECLARATION_KIND, FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND, IDENTIFIER_KIND,
        PARAMETER_DECLARATION_KIND, PREPROC_DEF_KIND, PREPROC_FUNCTION_DEF_KIND,
        STORAGE_CLASS_SPECIFIER_KIND, TYPE_IDENTIFIER_KIND, language,
    };

    #[test]
    fn kind_ids() {
        let l = language();
        assert_eq!(IDENTIFIER_KIND, l.id_for_node_kind("identifier", true));
        assert_eq!(
            FUNCTION_DEFINITION_KIND,
            l.id_for_node_kind("function_definition", true)
        );
        assert_eq!(DECLARATION_KIND, l.id_for_node_kind("declaration", true));
        assert_eq!(
            FUNCTION_DECLARATOR_KIND,
            l.id_for_node_kind("function_declarator", true)
        );
        assert_eq!(
            STORAGE_CLASS_SPECIFIER_KIND,
            l.id_for_node_kind("storage_class_specifier", true)
        );
        assert_eq!(
            PARAMETER_DECLARATION_KIND,
            l.id_for_node_kind("parameter_declaration", true)
        );
        assert_eq!(
            TYPE_IDENTIFIER_KIND,
            l.id_for_node_kind("type_identifier", true)
        );
        assert_eq!(PREPROC_DEF_KIND, l.id_for_node_kind("preproc_def", true));
        assert_eq!(
            PREPROC_FUNCTION_DEF_KIND,
            l.id_for_node_kind("preproc_function_def", true)
        );
    }
}
