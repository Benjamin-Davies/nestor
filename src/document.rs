use bytes::Bytes;
use lsp_types::TextDocumentItem;

use crate::analyze::{
    locals, parse,
    types::{Point, Range},
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

    pub fn find_references(&self, ident: tree_sitter::Node) -> Vec<Range> {
        let ident_bytes = self.source.slice(ident.byte_range());

        let symbols = self.locals.symbols.get(&ident_bytes);

        symbols.cloned().unwrap_or_default()
    }
}
