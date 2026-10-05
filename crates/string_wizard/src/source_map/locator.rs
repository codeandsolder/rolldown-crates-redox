#[derive(Debug)]
pub struct Locator {
  /// offsets are calculated based on utf-16
  line_offsets: Box<[u32]>,
}

impl Locator {
  #[expect(
    clippy::cast_possible_truncation,
    reason = "Locator is constructed for MagicString sources bounded to u32 offsets"
  )]
  pub fn new(source: &str) -> Self {
    let mut line_offsets = vec![];
    let mut line_start_pos: u32 = 0;
    for line in source.split('\n') {
      line_offsets.push(line_start_pos);
      // Fast path: ASCII lines have 1:1 byte-to-UTF-16 mapping
      let utf16_len = if line.is_ascii() {
        line.len() as u32
      } else {
        line.chars().map(|c| c.len_utf16() as u32).sum::<u32>()
      };
      line_start_pos += 1 + utf16_len;
    }
    Self { line_offsets: line_offsets.into_boxed_slice() }
  }

  /// Pass the index based on utf-16 and return the [Location] based on utf-16
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the line table cannot contain more entries than the bounded source has bytes"
  )]
  pub fn locate(&self, index: u32) -> Location {
    let mut left_cursor = 0;
    let mut right_cursor = self.line_offsets.len();
    while left_cursor < right_cursor {
      let mid = usize::midpoint(left_cursor, right_cursor);
      if index < self.line_offsets[mid] {
        right_cursor = mid;
      } else {
        left_cursor = mid + 1;
      }
    }
    let line = (left_cursor - 1) as u32;
    let column = index - self.line_offsets[left_cursor - 1];
    Location { line, column }
  }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Location {
  pub line: u32,
  // columns are calculated based on utf-16
  pub column: u32,
}

impl Location {
  pub const fn bump_line(&mut self) {
    self.line += 1;
    self.column = 0;
  }
}

#[test]
fn basic() {
  let source = "string\nwizard";
  let locator = Locator::new(source);

  assert_eq!(locator.line_offsets[0], 0);
  assert_eq!(locator.line_offsets[1], 7);

  assert_eq!(locator.locate(0), Location { line: 0, column: 0 });
  assert_eq!(locator.locate(12), Location { line: 1, column: 5 });
  assert_eq!(locator.locate(7), Location { line: 1, column: 0 });
  assert_eq!(locator.locate(1), Location { line: 0, column: 1 });
  assert_eq!(locator.locate(8), Location { line: 1, column: 1 });
}

#[test]
fn special_chars() {
  let source = "ß💣\n💣ß";
  let locator = Locator::new(source);
  assert_eq!(locator.line_offsets[0], 0);
  assert_eq!(locator.line_offsets[1], 4);

  assert_eq!(locator.locate(0), Location { line: 0, column: 0 });
  assert_eq!(locator.locate(4), Location { line: 1, column: 0 });
  assert_eq!(locator.locate(6), Location { line: 1, column: 2 });
}

#[test]
fn edge_cases() {
  let locator = Locator::new("");
  assert_eq!(locator.line_offsets.len(), 1);
  assert_eq!(locator.locate(0), Location { line: 0, column: 0 });
}
