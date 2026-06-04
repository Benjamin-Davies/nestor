use itertools::Itertools;
use nestor::scanner::{Keyword, Scanner, Token};

const BTREE_C: &str = include_str!("btree.c");

#[test]
fn test_scan_btree() {
    let tokens = Scanner::new(BTREE_C).collect_vec();

    assert_eq!(tokens.len(), 55924);
    assert_eq!(
        tokens
            .iter()
            .filter(|&&t| matches!(t, Token::Ident(_)))
            .count(),
        17490
    );
    assert_eq!(
        tokens
            .iter()
            .filter(|&&t| matches!(t, Token::Keyword(_)))
            .count(),
        2898
    );
    assert_eq!(
        tokens
            .iter()
            .filter(|&&t| t == Token::Keyword(Keyword::Static))
            .count(),
        143
    );
    assert_eq!(tokens.iter().filter(|&&t| t == Token::LBrace).count(), 1286);
    assert_eq!(
        tokens.iter().filter(|&&t| t == Token::Semicolon).count(),
        4347
    );
}
