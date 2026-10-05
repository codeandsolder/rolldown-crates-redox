use std::ops::{Index, IndexMut};

use crate::chunk::{Chunk, ChunkIdx};

#[derive(Debug, Clone)]
pub struct IndexChunks<'text>(Vec<Chunk<'text>>);

impl<'text> IndexChunks<'text> {
  pub fn with_capacity(capacity: usize) -> Self {
    Self(Vec::with_capacity(capacity))
  }

  pub fn push(&mut self, chunk: Chunk<'text>) -> ChunkIdx {
    let index = ChunkIdx::from_usize(self.0.len());
    self.0.push(chunk);
    index
  }
}

impl<'text> Index<ChunkIdx> for IndexChunks<'text> {
  type Output = Chunk<'text>;

  fn index(&self, index: ChunkIdx) -> &Self::Output {
    &self.0[index.as_usize()]
  }
}

impl<'text> IndexMut<ChunkIdx> for IndexChunks<'text> {
  fn index_mut(&mut self, index: ChunkIdx) -> &mut Self::Output {
    &mut self.0[index.as_usize()]
  }
}
