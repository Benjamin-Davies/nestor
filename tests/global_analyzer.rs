use std::collections::BTreeSet;

use nestor::analyze::{global::analyze, parse, types::Ident};

const BTREE_C: &[u8] = include_bytes!("btree.c");

#[test]
fn test_analyze() {
    let tree = parse(BTREE_C).unwrap();
    let globals = analyze(tree.root_node(), BTREE_C);

    assert_eq!(globals.symbols.len(), 13596);
    // fn call
    assert!(
        globals
            .symbols
            .iter()
            .any(|n| n.bytes == b"sqlite3BeginBenignMalloc")
    );

    assert_eq!(globals.definitions.len(), 83);
    let definitions = globals
        .definitions
        .iter()
        .map(Ident::to_str)
        .collect::<BTreeSet<_>>();
    // global fn
    assert!(definitions.contains("sqlite3BtreeOpen"));
    // static fn
    assert!(!definitions.contains("sharedLockTrace"));
    // global var
    assert!(definitions.contains("sqlite3BtreeTrace"));
    // static var
    assert!(!definitions.contains("zMagicHeader"));
    // local var
    assert!(!definitions.contains("zMsg"));
}
