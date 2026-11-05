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

mod kinds;
mod walk;

pub(crate) use kinds::*;

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

    use crate::analyze::parse;

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
