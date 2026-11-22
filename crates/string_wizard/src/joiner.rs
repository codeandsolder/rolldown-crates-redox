use crate::MagicString;

#[derive(Debug)]
pub struct JoinerOptions {
  pub separator: Option<String>,
}

#[derive(Debug, Default)]
pub struct Joiner<'s> {
  sources: Vec<MagicString<'s>>,
  separator: Option<String>,
}

impl<'s> Joiner<'s> {
  // --- public
  #[must_use = "the returned joiner owns the accumulated text"]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use = "the returned joiner owns the selected options"]
  pub fn with_options(options: JoinerOptions) -> Self {
    Self { separator: options.separator, ..Default::default() }
  }

  pub fn append(&mut self, source: MagicString<'s>) -> &mut Self {
    self.sources.push(source);
    self
  }

  pub fn append_raw(&mut self, raw: &'s str) -> &mut Self {
    self.sources.push(MagicString::new(raw));
    self
  }

  #[must_use = "the returned length describes the accumulated text"]
  pub fn len(&self) -> usize {
    self.fragments().map(str::len).sum()
  }

  #[must_use = "the returned status indicates whether any text has been accumulated"]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  #[must_use = "the joined text is returned rather than modifying caller state"]
  pub fn join(&self) -> String {
    let mut ret = String::with_capacity(self.len());
    self.fragments().for_each(|frag| {
      ret.push_str(frag);
    });
    ret
  }

  // --- private

  fn fragments(&'s self) -> impl Iterator<Item = &'s str> {
    let mut iter =
      self.sources.iter().flat_map(|c| self.separator.as_deref().into_iter().chain(c.fragments()));
    // Drop the first separator
    if self.separator.is_some() {
      let _ = iter.next();
    }
    iter
  }
}
