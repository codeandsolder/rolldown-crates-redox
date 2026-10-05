use crate::MagicString;

impl MagicString<'_> {
  /// Returns the content of the generated string that corresponds to the original
  /// positions from `start` to `end`.
  ///
  /// If `end` is `None`, it defaults to the original string length.
  ///
  /// # Errors
  /// Returns an error when a replaced character would be used as a slice anchor.
  pub fn slice(&self, start: u32, end: Option<u32>) -> Result<String, String> {
    let original_len = self.source_len;

    // Default end to original length
    let end = end.unwrap_or(original_len);

    let mut result = String::new();

    // Find start chunk
    let mut chunk_idx = Some(self.first_chunk_idx);
    while let Some(idx) = chunk_idx {
      let chunk = &self.chunks[idx];

      // Check if we're past start or at start
      if !(chunk.start() > start || chunk.end() <= start) {
        break;
      }

      // Found end chunk before start
      if chunk.start() < end && chunk.end() >= end {
        return Ok(result);
      }

      chunk_idx = chunk.next;
    }

    // Check if we found a chunk
    let Some(start_chunk_idx) = chunk_idx else {
      return Ok(result);
    };

    let start_chunk = &self.chunks[start_chunk_idx];
    if start_chunk.edited_content.is_some() && start_chunk.start() != start {
      return Err(format!("Cannot use replaced character {start} as slice start anchor."));
    }

    let mut chunk_idx = Some(start_chunk_idx);
    while let Some(idx) = chunk_idx {
      let chunk = &self.chunks[idx];

      // Add intro if this is not the start chunk, or if chunk.start === start
      if idx != start_chunk_idx || chunk.start() == start {
        for intro in &chunk.intro {
          result.push_str(intro.as_ref());
        }
      }

      let contains_end = chunk.start() < end && chunk.end() >= end;

      if contains_end && chunk.edited_content.is_some() && chunk.end() != end {
        return Err(format!("Cannot use replaced character {end} as slice end anchor."));
      }

      let slice_start = if idx == start_chunk_idx { (start - chunk.start()) as usize } else { 0 };

      let content =
        chunk.edited_content.as_deref().unwrap_or_else(|| chunk.span.text(&self.source));

      let slice_end = if contains_end {
        // end <= chunk.end() when contains_end is true, so we subtract
        content.len() - (chunk.end() - end) as usize
      } else {
        content.len()
      };

      // Guard against reversed range within a single chunk (matches JS .slice() behavior
      // which returns "" for reversed ranges instead of panicking).
      if slice_start < slice_end {
        result.push_str(&content[slice_start..slice_end]);
      }

      // Add outro if this chunk doesn't contain the end, or if chunk.end === end
      if !contains_end || chunk.end() == end {
        for outro in &chunk.outro {
          result.push_str(outro.as_ref());
        }
      }

      if contains_end {
        break;
      }

      chunk_idx = chunk.next;
    }

    Ok(result)
  }
}
