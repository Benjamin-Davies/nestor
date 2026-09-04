use crate::text::IncrementalChange;

impl IncrementalChange {
    // pub fn apply(&self, rope: &mut Rope, encoding: PositionEncoding) -> tree_sitter::InputEdit {
    //     let Range {
    //         start: start_char,
    //         end: old_end_char,
    //     } = self.old_range.to_char(rope, encoding);
    //     let tree_sitter::Range {
    //         start_byte,
    //         end_byte: old_end_byte,
    //         start_point: start_position,
    //         end_point: old_end_position,
    //     } = self.old_range.to_ts(rope, encoding);

    //     rope.remove(start_char..old_end_char);
    //     rope.insert(start_char, &self.new_text);

    //     let new_end_byte = start_byte + self.new_text.len();
    //     let new_end_position = byte_to_ts_point(new_end_byte, rope);

    //     tree_sitter::InputEdit {
    //         start_byte,
    //         old_end_byte,
    //         new_end_byte,
    //         start_position,
    //         old_end_position,
    //         new_end_position,
    //     }
    // }
}
