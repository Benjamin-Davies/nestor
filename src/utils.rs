use std::{
    cmp::Ordering,
    fmt,
    ops::Range,
    path::{Path, PathBuf},
};

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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Uri(url::Url);

impl Uri {
    pub fn from_abs_path(path: &Path) -> Self {
        let url = url::Url::from_file_path(path).expect("expected absolute path");
        Self(url)
    }

    pub fn as_path(&self) -> Option<PathBuf> {
        self.0.to_file_path().ok()
    }
}

impl From<lsp_types::Uri> for Uri {
    fn from(value: lsp_types::Uri) -> Self {
        Self(
            value
                .as_str()
                .parse()
                .expect("failed conversion from lsp_types::Uri to url::Url"),
        )
    }
}

impl From<Uri> for lsp_types::Uri {
    fn from(value: Uri) -> Self {
        value
            .0
            .as_str()
            .parse()
            .expect("failed conversion from url::Url to lsp_types::Uri")
    }
}

impl fmt::Debug for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::utils::{Uri, binary_search_range_by_key};

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

    #[test]
    fn convert_file_uri_to_path() {
        assert_eq!(Uri("http://example.com/".parse().unwrap()).as_path(), None);
        assert_eq!(Uri("file:///".parse().unwrap()).as_path(), Some("/".into()));
        assert_eq!(
            Uri("file:///home/bend/foo.txt".to_owned().parse().unwrap()).as_path(),
            Some("/home/bend/foo.txt".into()),
        );
    }

    #[test]
    fn convert_path_to_file_uri() {
        assert_eq!(
            Uri::from_abs_path(Path::new("/")),
            Uri("file:///".parse().unwrap())
        );
        assert_eq!(
            Uri::from_abs_path(Path::new("/home/bend/foo.txt")),
            Uri("file:///home/bend/foo.txt".parse().unwrap()),
        );
    }
}
