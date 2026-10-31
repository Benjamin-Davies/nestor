use bytes::Bytes;
use nestor::{
    analyze::{globals, parse},
    text::PositionEncoding,
};

const BTREE_C: &[u8] = include_bytes!("../../tests/btree.c");

fn main() {
    let source = Bytes::from_static(BTREE_C);
    let tree = parse(&source, None).unwrap();

    for _ in 0..10 {
        let globals = globals::analyze(
            std::hint::black_box(tree.root_node()),
            std::hint::black_box(source.clone()),
            PositionEncoding::Utf8,
        );
        std::hint::black_box(globals);
    }
}
