use std::ops::{Index, IndexMut};

use crate::chunk::{Chunk, ChunkIdx};

#[derive(Debug, Clone)]
pub struct IndexChunks<'text>(Vec<Chunk<'text>>);

impl<'text> IndexChunks<'text> {
  pub fn with_capacity(capacity: usize) -> Self {
    Self(Vec::with_capacity(capacity))
  }

  pub fn from_initial(chunk: Chunk<'text>) -> Self {
    Self(vec![chunk])
  }

  pub fn push(&mut self, chunk: Chunk<'text>) -> Result<ChunkIdx, String> {
    let index = ChunkIdx::from_usize(self.0.len())?;
    self.0.push(chunk);
    Ok(index)
  }
}

impl<'text> Index<ChunkIdx> for IndexChunks<'text> {
  type Output = Chunk<'text>;

  fn index(&self, index: ChunkIdx) -> &Self::Output {
    &self.0[index.as_usize()]
  }
}

impl IndexMut<ChunkIdx> for IndexChunks<'_> {
  fn index_mut(&mut self, index: ChunkIdx) -> &mut Self::Output {
    &mut self.0[index.as_usize()]
  }
}
