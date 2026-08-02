use bytes::Bytes;
use nestor::analyze::{
    globals, parse,
    types::{Ident, SymbolKind},
};

const BTREE_C: &[u8] = include_bytes!("btree.c");

#[test]
fn test_analyze() {
    let tree = parse(BTREE_C).unwrap();
    let globals = globals::analyze(tree.root_node(), Bytes::from(BTREE_C));

    assert_eq!(globals.symbols.len(), 13596);
    let find_sym = |s: &str| {
        globals
            .symbols
            .iter()
            .find(|ident| ident.bytes == s.as_bytes())
    };
    // fn call
    assert_eq!(
        find_sym("sqlite3BeginBenignMalloc"),
        Some(&Ident {
            bytes: "sqlite3BeginBenignMalloc".into(),
            range: "148:2-148:26".parse().unwrap(),
            kind: SymbolKind::Unknown
        })
    );

    assert_eq!(globals.type_symbols.len(), 991);

    assert_eq!(globals.definitions.len(), 116);
    let find_def = |s: &str| {
        globals
            .definitions
            .iter()
            .find(|ident| ident.bytes == s.as_bytes())
    };
    // global fn
    assert_eq!(
        find_def("sqlite3BtreeOpen"),
        Some(&Ident {
            bytes: "sqlite3BtreeOpen".into(),
            range: "2538:4-2538:20".parse().unwrap(),
            kind: SymbolKind::Function
        })
    );
    // static fn
    assert_eq!(find_def("sharedLockTrace"), None);
    // global var
    assert_eq!(
        find_def("sqlite3BtreeTrace"),
        Some(&Ident {
            bytes: "sqlite3BtreeTrace".into(),
            range: "39:4-39:21".parse().unwrap(),
            kind: SymbolKind::Variable
        })
    );
    // static var
    assert_eq!(find_def("zMagicHeader"), None);
    // local var
    assert_eq!(find_def("zMsg"), None);
    // macro
    assert_eq!(
        find_def("BTALLOC_ANY"),
        Some(&Ident {
            bytes: "BTALLOC_ANY".into(),
            range: "59:8-59:19".parse().unwrap(),
            kind: SymbolKind::Macro
        })
    );
    // function macro
    assert_eq!(
        find_def("TRACE"),
        Some(&Ident {
            bytes: "TRACE".into(),
            range: "40:9-40:14".parse().unwrap(),
            kind: SymbolKind::FunctionMacro
        })
    );

    assert_eq!(globals.type_definitions.len(), 6);
}
