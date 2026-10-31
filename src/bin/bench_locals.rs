use bytes_str::BytesStr;
use nestor::{
    analyze::{locals, parse},
    text::PositionEncoding,
};

const BTREE_C: &str = include_str!("../../tests/btree.c");

fn main() {
    let source = BytesStr::from_static(BTREE_C);
    let tree = parse(source.as_bytes(), None).unwrap();

    for _ in 0..10 {
        let locals = locals::analyze(
            std::hint::black_box(tree.root_node()),
            std::hint::black_box(&source),
            PositionEncoding::Utf8,
        );
        std::hint::black_box(locals);
    }
}
