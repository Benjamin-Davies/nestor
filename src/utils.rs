use std::{cmp::Ordering, ops::Range};

pub fn binary_search_range_by<'a, T, F>(slice: &'a [T], mut f: F) -> Range<usize>
where
    F: FnMut(&'a T) -> Ordering,
{
    // We cant use `<[T]>::partition_point` because the lifetimes aren't compatible.
    let start = slice
        .binary_search_by(|x| match f(x) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater | Ordering::Equal => Ordering::Greater,
        })
        .expect_err("closure should never return eq");

    let end = slice
        .binary_search_by(|x| match f(x) {
            Ordering::Less | Ordering::Equal => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
        })
        .expect_err("closure should never return eq");

    start..end
}

pub fn binary_search_range_by_key<'a, T, B, F>(slice: &'a [T], key: &B, mut f: F) -> Range<usize>
where
    F: FnMut(&'a T) -> B,
    B: Ord,
{
    binary_search_range_by(slice, |x| f(x).cmp(key))
}

#[cfg(test)]
mod tests {
    use crate::utils::binary_search_range_by_key;

    const NUMBERS: &[u32] = &[11, 12, 13, 14, 15];

    #[test]
    fn binary_search_no_match_lt() {
        let result = binary_search_range_by_key(NUMBERS, &2, |x| x / 2);
        assert_eq!(result, 0..0);
    }

    #[test]
    fn binary_search_no_match_gt() {
        let result = binary_search_range_by_key(NUMBERS, &10, |x| x / 2);
        assert_eq!(result, 5..5);
    }

    #[test]
    fn binary_search_matches() {
        let result = binary_search_range_by_key(NUMBERS, &6, |x| x / 2);
        assert_eq!(result, 1..3);
    }
}
