use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::analyze::{globals, parse};

/// Returns the number of definitions found
pub fn analyze(path: &Path) -> anyhow::Result<usize> {
    let n_defs = Arc::new(AtomicUsize::new(0));

    rayon::scope(|s| analyze_inner(s, path, n_defs.clone()))?;

    Ok(n_defs.load(Ordering::Acquire))
}

fn analyze_inner(
    scope: &rayon::Scope,
    path: &Path,
    n_defs: Arc<AtomicUsize>,
) -> anyhow::Result<()> {
    let metadata = path.metadata()?;
    if metadata.is_file() {
        if is_c_file(path) {
            let subtotal = analyze_file(path)?;
            n_defs.fetch_add(subtotal, Ordering::AcqRel);
        }
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let n_defs = n_defs.clone();

            scope.spawn(move |s| match analyze_inner(s, &entry_path, n_defs) {
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

fn analyze_file(path: &Path) -> anyhow::Result<usize> {
    let source = fs::read(path)?;
    let tree = parse(&source)?;

    let globals = globals::analyze(tree.root_node(), &source);

    Ok(globals.definitions.len())
}
