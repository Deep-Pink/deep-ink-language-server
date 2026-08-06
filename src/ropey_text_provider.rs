use ropey::Rope;
use std::cmp::min;
use tree_sitter::{Node, TextProvider};
pub struct RopeyTextProvider {
    rope: Rope,
}

impl RopeyTextProvider {
    pub fn new(rope: ropey::Rope) -> RopeyTextProvider {
        RopeyTextProvider { rope }
    }
}

pub struct RopeyIterator {
    pub rope: Rope,
    pub start_byte: usize,
    pub end_byte: usize,
}

impl Iterator for RopeyIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        const SLICE_LENGTH: usize = 256;
        let end = min(self.start_byte + SLICE_LENGTH, self.end_byte);
        let slice = self.rope.byte_slice(self.start_byte..end);
        let length = slice.len_bytes();
        if length == 0 {
            return None;
        }
        self.start_byte = end;
        let str = slice.to_string();
        return Some(str);
    }
}

impl TextProvider<String> for RopeyTextProvider {
    type I = RopeyIterator;

    fn text(&mut self, node: Node) -> Self::I {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let rope = self.rope.clone();
        RopeyIterator {
            rope,
            start_byte,
            end_byte,
        }
    }
}
