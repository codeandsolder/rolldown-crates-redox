use std::{collections::VecDeque, num::NonZeroU32};

use crate::{CowStr, span::Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkIdx(NonZeroU32);

impl ChunkIdx {
  pub(crate) fn from_usize(index: usize) -> Self {
    let encoded = u32::try_from(index + 1).expect("chunk index exceeds the 4GB source limit");
    Self(NonZeroU32::new(encoded).expect("chunk indices are encoded one-based"))
  }

  pub(crate) fn as_usize(self) -> usize {
    self.0.get() as usize - 1
  }
}

#[derive(Debug, Clone, Copy)]
pub struct EditOptions {
  /// `true` will clear the `intro` and `outro` of the [Chunk]
  pub overwrite: bool,
  pub store_name: bool,
}

impl Default for EditOptions {
  fn default() -> Self {
    Self { overwrite: true, store_name: false }
  }
}

#[derive(Debug, Default, Clone)]
pub struct Chunk<'str> {
  pub intro: VecDeque<CowStr<'str>>,
  pub outro: VecDeque<CowStr<'str>>,
  pub span: Span,
  pub edited_content: Option<CowStr<'str>>,
  pub next: Option<ChunkIdx>,
  pub prev: Option<ChunkIdx>,
  pub keep_in_mappings: bool,
}

impl Chunk<'_> {
  pub fn new(span: Span) -> Self {
    Self { span, ..Default::default() }
  }
}

impl<'str> Chunk<'str> {
  pub fn start(&self) -> u32 {
    self.span.start()
  }

  pub fn end(&self) -> u32 {
    self.span.end()
  }

  pub fn contains(&self, text_index: u32) -> bool {
    self.start() < text_index && text_index < self.end()
  }

  pub fn append_outro(&mut self, content: CowStr<'str>) {
    self.outro.push_back(content)
  }

  pub fn append_intro(&mut self, content: CowStr<'str>) {
    self.intro.push_back(content)
  }

  pub fn prepend_outro(&mut self, content: CowStr<'str>) {
    self.outro.push_front(content)
  }

  pub fn prepend_intro(&mut self, content: CowStr<'str>) {
    self.intro.push_front(content)
  }

  pub fn split<'a>(&'a mut self, text_index: u32) -> Result<Chunk<'str>, String> {
    if let Some(ref content) = self.edited_content
      && !content.is_empty()
    {
      return Err("Cannot split a chunk that has already been edited".to_string());
    }
    let first_half_slice = Span(self.start(), text_index);
    let second_half_slice = Span(text_index, self.end());
    let mut new_chunk = Chunk::new(second_half_slice);
    if self.is_edited() {
      new_chunk
        .edit("".into(), EditOptions { store_name: self.keep_in_mappings, overwrite: false });
    }
    std::mem::swap(&mut new_chunk.outro, &mut self.outro);
    self.span = first_half_slice;
    Ok(new_chunk)
  }

  pub fn fragments(&'str self, original_source: &'str str) -> impl Iterator<Item = &'str str> {
    let intro_iter = self.intro.iter().map(|frag| frag.as_ref());
    let source_frag = self
      .edited_content
      .as_ref()
      .map(|s| s.as_ref())
      .unwrap_or_else(|| self.span.text(original_source));
    let outro_iter = self.outro.iter().map(|frag| frag.as_ref());
    intro_iter.chain(Some(source_frag)).chain(outro_iter)
  }

  pub fn edit(&mut self, content: CowStr<'str>, opts: EditOptions) {
    if opts.overwrite {
      self.intro.clear();
      self.outro.clear();
    }
    self.keep_in_mappings = opts.store_name;
    self.edited_content = Some(content);
  }

  pub fn is_edited(&self) -> bool {
    self.edited_content.is_some()
  }

  /// Resets the chunk to its original state.
  /// Clears intro and outro, and if the chunk was edited, restores the original content.
  pub fn reset(&mut self) {
    self.intro.clear();
    self.outro.clear();
    if self.is_edited() {
      self.edited_content = None;
      self.keep_in_mappings = false;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn optional_chunk_index_stays_compact() {
    assert_eq!(std::mem::size_of::<Option<ChunkIdx>>(), std::mem::size_of::<u32>());
  }
}
