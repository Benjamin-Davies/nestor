use bytes::Bytes;
use lsp_types::TextDocumentItem;

use crate::analyze::{
    locals, parse,
    types::{Ident, Point, Range},
};

pub struct Document {
    source: Bytes,
    tree: tree_sitter::Tree,
    locals: locals::Locals,
}

impl TryFrom<TextDocumentItem> for Document {
    type Error = anyhow::Error;

    fn try_from(text_document: TextDocumentItem) -> anyhow::Result<Self> {
        let source = Bytes::from(text_document.text);

        let tree = parse(&source)?;

        let locals = locals::analyze(tree.root_node(), &source);

        Ok(Self {
            source,
            tree,
            locals,
        })
    }
}

impl Document {
    pub fn ident_at(&self, point: Point) -> Option<tree_sitter::Node<'_>> {
        let ts_point = tree_sitter::Point::from(point);

        let node = self
            .tree
            .root_node()
            .descendant_for_point_range(ts_point, ts_point)?;

        // TODO: numeric ID?
        if node.kind() == "identifier" {
            Some(node)
        } else {
            None
        }
    }

    /// Returns the locations of the references we found and a boolean
    /// to indicate if the symbol is a local variable.
    pub fn find_references(&self, ident: tree_sitter::Node) -> (Vec<Range>, bool) {
        let ident = Ident {
            bytes: self.source.slice(ident.byte_range()),
            range: ident.range().into(),
        };

        let symbols = self.locals.symbols.get(&ident.bytes);

        let mut definitions = self.locals.definitions(ident);
        definitions.retain(|d| !d.is_function);
        if !definitions.is_empty() {
            // If there are local vars that match, assume that the user wants
            // references to those vars.
            let matches = symbols
                .into_iter()
                .flatten()
                .copied()
                .filter(|&s| definitions.iter().any(|d| d.scope.contains(s)))
                .collect();
            (matches, true)
        } else {
            let matches = symbols.cloned().unwrap_or_default();
            (matches, false)
        }
    }

    pub fn find_definitions(&self, ident: tree_sitter::Node) -> Vec<Range> {
        let ident = Ident {
            bytes: self.source.slice(ident.byte_range()),
            range: ident.range().into(),
        };

        let definitions = self.locals.definitions(ident);

        definitions.into_iter().map(|d| d.name).collect()
    }

    pub fn bytes_for<'a>(&'a self, node: tree_sitter::Node) -> &'a [u8] {
        &self.source[node.byte_range()]
    }
}
