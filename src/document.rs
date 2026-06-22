//! Document store and LSP position <-> byte-offset conversion.
//!
//! LSP `Position.character` is a UTF-16 code-unit offset within a line. The
//! tokenizer works in byte offsets, so every conversion routes through
//! `ropey`'s UTF-16 helpers — never `byte_of_line + character`, which is only
//! correct for ASCII.

use dashmap::DashMap;
use ropey::Rope;
use tower_lsp_server::ls_types::Position;
use tower_lsp_server::ls_types::Range;
use tower_lsp_server::ls_types::Uri;

#[derive(Default)]
pub struct DocumentStore {
    docs: DashMap<Uri, Rope>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&self, uri: Uri, text: &str) {
        self.docs.insert(uri, Rope::from_str(text));
    }

    pub fn remove(&self, uri: &Uri) {
        self.docs.remove(uri);
    }

    /// Clone the `Rope` (cheap, ref-counted) so callers never hold a `DashMap`
    /// guard across an `.await`.
    pub fn rope(&self, uri: &Uri) -> Option<Rope> {
        self.docs.get(uri).map(|r| r.clone())
    }
}

/// Convert an LSP position to a byte offset in the rope, or `None` if the
/// position is out of range.
pub fn position_to_byte(rope: &Rope, pos: Position) -> Option<usize> {
    let line = pos.line as usize;
    if line >= rope.len_lines() {
        return None;
    }
    let line_start_char = rope.line_to_char(line);
    let line_start_u16 = rope.char_to_utf16_cu(line_start_char);
    let target_u16 = (line_start_u16 + pos.character as usize).min(rope.len_utf16_cu());
    let char_idx = rope.utf16_cu_to_char(target_u16);
    Some(rope.char_to_byte(char_idx))
}

/// Convert a byte offset to an LSP position.
pub fn byte_to_position(rope: &Rope, byte: usize) -> Position {
    let byte = byte.min(rope.len_bytes());
    let char_idx = rope.byte_to_char(byte);
    let line = rope.char_to_line(char_idx);
    let line_start_char = rope.line_to_char(line);
    let col_u16 = rope.char_to_utf16_cu(char_idx) - rope.char_to_utf16_cu(line_start_char);
    Position::new(line as u32, col_u16 as u32)
}

/// Build an LSP range from a byte span.
pub fn span_to_range(rope: &Rope, start: usize, end: usize) -> Range {
    Range::new(byte_to_position(rope, start), byte_to_position(rope, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ascii_and_unicode() {
        // emoji (2 UTF-16 cu), accented, dotted-I, sharp-s, multi-line.
        let text = "x🚀é\nİß @foo";
        let rope = Rope::from_str(text);
        for (b, _) in text
            .char_indices()
            .chain(std::iter::once((text.len(), ' ')))
        {
            let pos = byte_to_position(&rope, b);
            let back = position_to_byte(&rope, pos).expect("in range");
            assert_eq!(back, b, "byte {b} did not round-trip via {pos:?}");
        }
    }

    #[test]
    fn emoji_is_two_utf16_units() {
        let rope = Rope::from_str("🚀x");
        // After the rocket (4 bytes) the column should be 2 UTF-16 code units.
        let pos = byte_to_position(&rope, 4);
        assert_eq!(pos, Position::new(0, 2));
    }
}
