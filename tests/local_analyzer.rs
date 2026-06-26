use bytes::Bytes;
use nestor::analyze::{locals, parse};

const BTREE_C: &[u8] = include_bytes!("btree.c");

#[test]
fn test_analyze() {
    let source = Bytes::from(BTREE_C);
    let tree = parse(&source).unwrap();
    let locals = locals::analyze(tree.root_node(), &source);

    assert_eq!(locals.symbols.len(), 1003);
    assert_eq!(locals.symbols.get(b"pPage".as_slice()).unwrap().len(), 840);
}
