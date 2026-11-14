use std::{env, fmt, fs};

use bytes::Bytes;
use bytes_str::BytesStr;
use nestor::{
    analyze::{globals, hover, locals, parse},
    text::{PositionEncoding, buffer::TextBuffer},
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

#[test]
fn test_hover_variable() {
    let source = TextBuffer::from(BTREE_C.to_owned());
    let tree = parse(BTREE_C.as_bytes(), None).unwrap();
    let hover = hover::hover(
        &source,
        PositionEncoding::Utf8,
        &tree,
        "32:18-32:20".parse().unwrap(),
    );

    assert_expected("hover_variable", &hover);
}

#[test]
fn test_hover_function() {
    let source = TextBuffer::from(BTREE_C.to_owned());
    let tree = parse(BTREE_C.as_bytes(), None).unwrap();
    let hover = hover::hover(
        &source,
        PositionEncoding::Utf8,
        &tree,
        "99:4-99:31".parse().unwrap(),
    );

    assert_expected("hover_function", &hover);
}

#[test]
fn test_hover_macro() {
    let source = TextBuffer::from(BTREE_C.to_owned());
    let tree = parse(BTREE_C.as_bytes(), None).unwrap();
    let hover = hover::hover(
        &source,
        PositionEncoding::Utf8,
        &tree,
        "54:8-54:23".parse().unwrap(),
    );

    assert_expected("hover_macro", &hover);
}

#[test]
fn test_hover_comment_before() {
    let source = TextBuffer::from(SMALL_C.to_owned());
    let tree = parse(SMALL_C.as_bytes(), None).unwrap();
    let hover = hover::hover(
        &source,
        PositionEncoding::Utf8,
        &tree,
        "213:4-213:8".parse().unwrap(),
    );

    assert_expected("hover_comment_before", &hover);
}

#[test]
fn test_hover_comment_after() {
    let source = TextBuffer::from(SMALL_C.to_owned());
    let tree = parse(SMALL_C.as_bytes(), None).unwrap();
    let hover = hover::hover(
        &source,
        PositionEncoding::Utf8,
        &tree,
        "218:2-218:5".parse().unwrap(),
    );

    assert_expected("hover_comment_after", &hover);
}

#[test]
fn test_hover_comment_multiple_lines() {
    let source = TextBuffer::from(SMALL_C.to_owned());
    let tree = parse(SMALL_C.as_bytes(), None).unwrap();
    let hover = hover::hover(
        &source,
        PositionEncoding::Utf8,
        &tree,
        "217:2-217:5".parse().unwrap(),
    );

    assert_expected("hover_comment_multiple_lines", &hover);
}

#[test]
fn test_hover_comment_none() {
    let source = TextBuffer::from(SMALL_C.to_owned());
    let tree = parse(SMALL_C.as_bytes(), None).unwrap();
    let hover = hover::hover(
        &source,
        PositionEncoding::Utf8,
        &tree,
        "214:2-214:5".parse().unwrap(),
    );

    assert_expected("hover_comment_none", &hover);
}

fn assert_expected(key: &str, value: impl fmt::Debug) {
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
