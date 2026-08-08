use nestor::analyze::{
    locals::{self, Definition},
    parse_rope,
    types::SymbolKind,
};
use ropey::Rope;

const BTREE_C: &str = include_str!("btree.c");

#[test]
fn test_analyze_symbols() {
    let source = Rope::from(BTREE_C);
    let tree = parse_rope(&source, None).unwrap();
    let locals = locals::analyze(tree.root_node(), &source.into());

    assert_eq!(locals.symbols.len(), 1003);
    assert_eq!(locals.symbols.get(b"pPage".as_slice()).unwrap().len(), 840);
}

#[test]
fn test_analyze_definition_count() {
    let source = Rope::from(BTREE_C);
    let tree = parse_rope(&source, None).unwrap();
    let locals = locals::analyze(tree.root_node(), &source);

    assert_eq!(locals.definitions.len(), 679);
}

#[test]
fn test_analyze_function_definition() {
    let source = Rope::from(BTREE_C);
    let tree = parse_rope(&source, None).unwrap();
    let locals = locals::analyze(tree.root_node(), &source);

    assert_eq!(
        locals
            .definitions
            .get(b"sqlite3BtreeSeekCount".as_slice())
            .unwrap(),
        &[Definition {
            name: "129:15-129:36".parse().unwrap(),
            scope: "1:0-11601:0".parse().unwrap(),
            kind: SymbolKind::Function
        }]
    );
}
