#[derive(Debug, Default, Clone, Copy)]
pub struct Span(pub u32, pub u32);

impl Span {
  pub const fn start(self) -> u32 {
    self.0
  }

  pub const fn end(self) -> u32 {
    self.1
  }

  pub fn text(self, source: &str) -> &str {
    &source[self.start() as usize..self.end() as usize]
  }
}
