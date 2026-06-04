use criterion::{Criterion, criterion_group, criterion_main};
use nestor::scanner::Scanner;
use std::hint::black_box;

const BTREE_C: &str = include_str!("../tests/btree.c");

fn scan_btree_c(c: &mut Criterion) {
    c.bench_function("scan btree.c", |b| {
        b.iter(|| Scanner::new(black_box(BTREE_C)).count())
    });
}

criterion_group!(benches, scan_btree_c);
criterion_main!(benches);
