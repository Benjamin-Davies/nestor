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

const IDENTIFIER_KIND: u16 = 1;
const FUNCTION_DEFINITION_KIND: u16 = 196;
const FUNCTION_DECLARATOR_KIND: u16 = 230;

#[cfg(test)]
mod tests {
    use crate::analyze::{
        FUNCTION_DECLARATOR_KIND, FUNCTION_DEFINITION_KIND, IDENTIFIER_KIND, language,
    };

    #[test]
    fn kind_ids() {
        let l = language();
        assert_eq!(IDENTIFIER_KIND, l.id_for_node_kind("identifier", true));
        assert_eq!(
            FUNCTION_DEFINITION_KIND,
            l.id_for_node_kind("function_definition", true)
        );
        assert_eq!(
            FUNCTION_DECLARATOR_KIND,
            l.id_for_node_kind("function_declarator", true)
        );
    }
}
