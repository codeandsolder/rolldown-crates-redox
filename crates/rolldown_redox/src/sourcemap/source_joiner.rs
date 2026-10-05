// Adapted from rolldown/rolldown crates/rolldown_sourcemap (MIT).
use oxc_sourcemap::ConcatSourceMapBuilder;

use super::{SourceMap, source::Source};

#[derive(Debug, Default)]
pub struct SourceJoiner<'source> {
  inner: Vec<Box<dyn Source + Send + 'source>>,
  prepend_source: Vec<Box<dyn Source + Send + 'source>>,
  pub enable_sourcemap: bool,
  names_len: usize,
  sources_len: usize,
  tokens_len: usize,
  token_chunks_len: usize,
}

impl<'source> SourceJoiner<'source> {
  pub fn append_source<T: Source + Send + 'source>(&mut self, source: T) {
    if let Some(sourcemap) = source.sourcemap() {
      self.accumulate_sourcemap_data_size(sourcemap);
    }
    self.inner.push(Box::new(source));
  }

  pub fn append_source_dyn(&mut self, source: Box<dyn Source + Send + 'source>) {
    if let Some(sourcemap) = source.sourcemap() {
      self.accumulate_sourcemap_data_size(sourcemap);
    }
    self.inner.push(source);
  }

  pub fn prepend_source<T: Source + Send + 'source>(&mut self, source: T) {
    if let Some(sourcemap) = source.sourcemap() {
      self.accumulate_sourcemap_data_size(sourcemap);
    }
    self.prepend_source.push(Box::new(source));
  }

  pub fn join(&mut self) -> (String, Option<SourceMap>) {
    let sources_len = self.prepend_source.len() + self.inner.len();

    let size_hint_of_ret_source = self
      .prepend_source
      .iter()
      .chain(self.inner.iter())
      .map(|source| source.content().len())
      .sum::<usize>()
      + sources_len;
    let mut ret_source = String::with_capacity(size_hint_of_ret_source);

    let mut sourcemap_builder = self.enable_sourcemap.then(|| {
      ConcatSourceMapBuilder::with_capacity(
        self.names_len,
        self.sources_len,
        self.tokens_len,
        self.token_chunks_len,
      )
    });
    if let Some(sourcemap_builder) = &mut sourcemap_builder {
      let mut line_offset = 0;
      // Move exclusively owned maps into the builder. A caller may still pass
      // a shared source by reference; keep borrowing that map so its mappings
      // are preserved, then copy only its borrowed strings when detaching.
      for (index, source) in self.prepend_source.iter_mut().chain(self.inner.iter_mut()).enumerate()
      {
        ret_source.push_str(source.content());
        if let Some(map) = source.take_sourcemap() {
          sourcemap_builder.add_sourcemap_owned(map, line_offset);
        } else if let Some(map) = source.sourcemap() {
          sourcemap_builder.add_sourcemap(map, line_offset);
        }
        // The line count only advances the offset for a *following* source, so
        // scan it inside this branch — never for the final source, which can be
        // a whole chunk appended without a pre-computed count.
        if index < sources_len - 1 {
          ret_source.push('\n');
          line_offset += source.lines_count() + 1; // +1 for the newline
        }
      }
    } else {
      // Without a sourcemap there is no line offset to maintain. Avoid scanning
      // every source for newlines on this common path.
      for (index, source) in self.prepend_source.iter().chain(self.inner.iter()).enumerate() {
        ret_source.push_str(source.content());
        if index < sources_len - 1 {
          ret_source.push('\n');
        }
      }
    }
    (ret_source, sourcemap_builder.map(|builder| builder.into_owned_sourcemap().into_inner()))
  }

  fn accumulate_sourcemap_data_size(&mut self, hint: &SourceMap) {
    self.enable_sourcemap = true;
    self.names_len += hint.get_names().count();
    self.sources_len += hint.get_sources().count();
    self.tokens_len += hint.get_tokens().count();
    self.token_chunks_len += 1;
  }
}
