use tree_sitter::Node;

/// Iterates over the descendants of a node in pre-order, skipping the node itself.
pub fn walk_preorder<'a>(node: Node<'a>) -> impl Iterator<Item = Node<'a>> {
    let mut cursor = node.walk();

    std::iter::from_fn(move || {
        if cursor.goto_first_child() {
            return Some(cursor.node());
        }

        loop {
            if cursor.goto_next_sibling() {
                return Some(cursor.node());
            }

            if !cursor.goto_parent() {
                return None;
            }
        }
    })
}
