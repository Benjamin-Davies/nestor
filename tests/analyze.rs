use std::{env, fmt, fs};

use bytes::Bytes;
use bytes_str::BytesStr;
use nestor::{
    analyze::{globals, locals, parse},
    text::PositionEncoding,
};

const BTREE_C: &str = include_str!("btree.c");
const SMALL_C: &str = include_str!("small.c");

#[test]
fn test_analyze_globals() {
    let tree = parse(BTREE_C.as_bytes(), None).unwrap();
    let globals = globals::analyze(
        tree.root_node(),
        Bytes::from_static(BTREE_C.as_bytes()),
        PositionEncoding::Utf8,
    );

    assert_expected("globals_analyze_btree", &globals);
}

#[test]
fn test_analyze_locals() {
    let tree = parse(BTREE_C.as_bytes(), None).unwrap();
    let locals = locals::analyze(
        tree.root_node(),
        &BytesStr::from_static(BTREE_C),
        PositionEncoding::Utf8,
    );

    assert_expected("locals_analyze_btree", &locals);
}

#[test]
fn test_analyze_globals_small() {
    let tree = parse(SMALL_C.as_bytes(), None).unwrap();
    let globals = globals::analyze(
        tree.root_node(),
        Bytes::from_static(SMALL_C.as_bytes()),
        PositionEncoding::Utf8,
    );

    assert_expected("globals_analyze_small", &globals);
}

#[test]
fn test_analyze_locals_small() {
    let tree = parse(SMALL_C.as_bytes(), None).unwrap();
    let locals = locals::analyze(
        tree.root_node(),
        &BytesStr::from_static(SMALL_C),
        PositionEncoding::Utf8,
    );

    assert_expected("locals_analyze_small", &locals);
}

fn assert_expected(key: &str, value: impl fmt::Debug) {
    dbg!(key);
    let path = format!("tests/data/{key}.txt");
    let contents = format!("{value:#?}");

    if should_generate_test_data() {
        fs::write(path, contents).unwrap();
    } else {
        let expected_contents = fs::read_to_string(path).unwrap();
        if contents != expected_contents {
            panic!("{key} does not match expected value");
        }
    }
}

fn should_generate_test_data() -> bool {
    env::var("GENERATE_TEST_DATA").is_ok_and(|s| !s.is_empty())
}
