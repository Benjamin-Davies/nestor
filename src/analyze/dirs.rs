use std::{fs, path::Path, sync::Arc};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crossbeam_channel::{Sender, unbounded};
use itertools::Itertools;

use crate::analyze::{globals, parse, types::Range};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Symbol {
    pub name: Bytes,
    pub path: Arc<Path>,
    pub range: Range,
    pub is_definition: bool,
}

/// Returns the number of definitions found
pub fn analyze(path: &Path) -> Vec<Symbol> {
    rayon::scope(|s| {
        let (symbols_tx, symbols_rx) = unbounded();

        s.spawn(|s| {
            if let Err(err) = analyze_inner(s, path, symbols_tx) {
                tracing::error!("Error analyzing folder {path:?}: {err}");
            }
        });

        symbols_rx.iter().flatten().collect_vec()
    })
}

fn analyze_inner(
    scope: &rayon::Scope,
    path: &Path,
    symbols_tx: Sender<Vec<Symbol>>,
) -> anyhow::Result<()> {
    let metadata = path.metadata()?;
    if metadata.is_file() {
        if is_c_file(path) {
            let symbols = analyze_file(path)?;
            symbols_tx.send(symbols)?;
        }
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let symbols_tx = symbols_tx.clone();

            scope.spawn(move |s| match analyze_inner(s, &entry_path, symbols_tx) {
                Ok(()) => {}
                Err(err) => tracing::error!("Error analyzing dir: {err:?}"),
            });
        }
    } else {
        tracing::info!("Path does not point to file or dir: {path:?}");
    }

    Ok(())
}

fn is_c_file(path: &Path) -> bool {
    let extension = path.extension();
    extension == Some("c".as_ref()) || extension == Some("h".as_ref())
}

fn analyze_file(path: &Path) -> anyhow::Result<Vec<Symbol>> {
    let source = fs::read(path)?;
    let source = Bytes::from(source);
    let tree = parse(&source)?;

    let globals = globals::analyze(tree.root_node(), source);

    let path = Arc::<Path>::from(path);
    let mut symbols = Vec::with_capacity(globals.definitions.len() + globals.symbols.len());
    for ident in globals.definitions {
        symbols.push(Symbol {
            name: ident.bytes,
            path: path.clone(),
            range: ident.range,
            is_definition: true,
        });
    }
    for ident in globals.symbols {
        symbols.push(Symbol {
            name: ident.bytes,
            path: path.clone(),
            range: ident.range,
            is_definition: false,
        });
    }

    consolidate_symbols(&mut symbols);

    Ok(symbols)
}

/// Sorts symbols and consolidates their underlying bytes objects into a single smaller bytes object.
fn consolidate_symbols(symbols: &mut [Symbol]) {
    symbols.sort();

    let mut buf = BytesMut::new();
    let mut last_name: &[u8] = &[];
    for symbol in &*symbols {
        if symbol.name.starts_with(last_name) {
            buf.put(&symbol.name[last_name.len()..]);
        } else {
            buf.put(&*symbol.name)
        }
        last_name = &*symbol.name;
    }

    let mut buf = buf.freeze();
    let mut next_start = 0;
    for symbol in symbols {
        if !buf.starts_with(&symbol.name) {
            buf.advance(next_start);
            // This should always be the case, but check just in case.
            debug_assert!(buf.starts_with(&symbol.name));
        }
        symbol.name = buf.slice(..symbol.name.len());
        next_start = symbol.name.len();
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use bytes::Bytes;
    use itertools::Itertools;

    use crate::analyze::dirs::{Symbol, consolidate_symbols};

    #[test]
    fn consolidate_symbols_simple() {
        let names = &["a", "d", "abc", "ab"];
        let mut symbols = names
            .iter()
            .map(|name| Symbol {
                name: Bytes::from(*name),
                path: Path::new("").into(),
                range: Default::default(),
                is_definition: Default::default(),
            })
            .collect_vec();

        consolidate_symbols(&mut symbols);

        assert_eq!(symbols.len(), 4);
        let names = symbols.iter().map(|s| &*s.name).collect_vec();
        assert_eq!(&names, &[b"a" as &[u8], b"ab", b"abc", b"d"]);
    }

    #[test]
    fn consolidate_symbols_consolidates() {
        let names = &["a", "d", "abc", "ab"];
        let mut symbols = names
            .iter()
            .map(|name| Symbol {
                name: Bytes::from(*name),
                path: Path::new("").into(),
                range: Default::default(),
                is_definition: Default::default(),
            })
            .collect_vec();

        consolidate_symbols(&mut symbols);

        // The first 3 symbols should all point at slices of the same memory.
        let first_ptr = symbols[0].name.as_ptr();
        for symbol in &symbols[..3] {
            assert_eq!(symbol.name.as_ptr(), first_ptr);
        }
    }
}
