use itertools::Itertools;
use ropey::Rope;

use crate::analyze::{
    IDENTIFIER_KIND, TYPE_IDENTIFIER_KIND, locals, parse, parse_rope,
    types::{Ident, Point, Range, SymbolKind},
};

pub struct Document {
    source: Rope,
    tree: tree_sitter::Tree,
    locals: locals::Locals,
}

impl Document {
    pub fn parse(source: String) -> anyhow::Result<Self> {
        let tree = parse(source.as_bytes())?;

        let source = Rope::from(source);
        let locals = locals::analyze(tree.root_node(), &source);

        Ok(Self {
            source,
            tree,
            locals,
        })
    }

    pub fn update(&mut self, changes: Vec<(Range, String)>) -> anyhow::Result<()> {
        for (old_range, new_text) in changes {
            // TODO: Consider non-ASCII chars
            let start_index = self.source.line_to_char(old_range.start.row as usize)
                + old_range.start.column as usize;
            let end_index = self.source.line_to_char(old_range.end.row as usize)
                + old_range.end.column as usize;

            self.source.remove(start_index..end_index);
            self.source.insert(start_index, &new_text);
        }

        self.tree = parse_rope(&self.source, Some(&self.tree))?;

        self.locals = locals::analyze(self.tree.root_node(), &self.source);

        Ok(())
    }

    pub fn ident_at(&self, point: Point) -> Option<tree_sitter::Node<'_>> {
        let ts_point = tree_sitter::Point::from(point);

        let node = self
            .tree
            .root_node()
            .descendant_for_point_range(ts_point, ts_point)?;

        if let IDENTIFIER_KIND | TYPE_IDENTIFIER_KIND = node.kind_id() {
            Some(node)
        } else {
            None
        }
    }

    /// Returns the locations of the references we found and a boolean
    /// to indicate if the symbol is a local variable.
    pub fn find_references(&self, ident: tree_sitter::Node) -> (Vec<Range>, bool) {
        let ident = Ident::from_node_rope(ident, &self.source);

        let symbols = self.locals.symbols.get(&ident.bytes);

        let mut definitions = self.locals.definitions(ident);
        definitions.retain(|d| d.kind != SymbolKind::Function);
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
        let ident = Ident::from_node_rope(ident, &self.source);

        let definitions = self.locals.definitions(ident);

        definitions.into_iter().map(|d| d.name).collect()
    }

    pub fn bytes_for<'a>(&'a self, node: tree_sitter::Node) -> Vec<u8> {
        self.source
            .byte_slice(node.byte_range())
            .to_string()
            .into_bytes()
    }

    pub fn completions(&self, point: Point) -> Vec<(&[u8], SymbolKind)> {
        self.locals
            .definitions
            .iter()
            .flat_map(move |(name, defs)| {
                defs.iter()
                    .filter(move |def| def.scope.contains_point(point))
                    .map(|def| (name as &[_], def.kind))
            })
            .collect_vec()
    }
}
