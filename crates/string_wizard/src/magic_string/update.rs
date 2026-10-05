use std::borrow::Cow;

use crate::{CowStr, MagicString, chunk::EditOptions};

#[derive(Debug, Default, Clone)]
pub struct UpdateOptions {
  /// `true` will store the original content in the `name` field of the generated sourcemap.
  pub keep_original: bool,

  /// `true` will clear the `intro` and `outro` for the corresponding range.
  pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct TextEdit<'text> {
  pub start: usize,
  pub end: usize,
  pub content: Cow<'text, str>,
  pub options: UpdateOptions,
}

impl<'text> TextEdit<'text> {
  pub fn new(start: usize, end: usize, content: impl Into<Cow<'text, str>>) -> Self {
    Self { start, end, content: content.into(), options: UpdateOptions::default() }
  }

  pub fn with_options(
    start: usize,
    end: usize,
    content: impl Into<Cow<'text, str>>,
    options: UpdateOptions,
  ) -> Self {
    Self { start, end, content: content.into(), options }
  }

  /// Insert content before the original-source byte at `offset`.
  ///
  /// Multiple insertions at the same offset preserve their input order when
  /// passed to [`MagicString::apply_edits`].
  #[must_use]
  pub fn insert(offset: usize, content: impl Into<Cow<'text, str>>) -> Self {
    Self::new(offset, offset, content)
  }

  /// Remove the original-source range `start..end`.
  #[must_use]
  pub fn remove(start: usize, end: usize) -> Self {
    Self::new(start, end, "")
  }
}

impl<'text> MagicString<'text> {
  /// A shorthand for `update_with(start, end, content, UpdateOptions::default())`.
  ///
  /// # Errors
  /// Returns an error when the requested source range cannot be updated safely.
  pub fn update(
    &mut self,
    start: u32,
    end: u32,
    content: impl Into<CowStr<'text>>,
  ) -> Result<&mut Self, String> {
    self.update_with(start, end, content, UpdateOptions::default())
  }

  /// # Errors
  /// Returns an error when the requested source range cannot be updated safely.
  pub fn update_with(
    &mut self,
    start: u32,
    end: u32,
    content: impl Into<CowStr<'text>>,
    opts: UpdateOptions,
  ) -> Result<&mut Self, String> {
    self.inner_update_with(start, end, content.into(), opts, true)
  }

  /// Applies a set of non-overlapping original-source edits atomically.
  ///
  /// Edits use UTF-8 byte offsets and may be provided in any order. A
  /// zero-length range is an insertion before the original-source byte at that
  /// offset. Insertions may share an offset and may sit exactly on a replacement
  /// boundary, but an insertion strictly inside a replaced range is rejected.
  /// Multiple insertions at one offset preserve caller order.
  ///
  /// All ranges are checked before mutation; if validation or any update fails,
  /// `self` remains unchanged.
  ///
  /// # Errors
  /// Returns an error for invalid, overlapping, out-of-bounds, or unrepresentable edits.
  pub fn apply_edits(
    &mut self,
    edits: impl IntoIterator<Item = TextEdit<'text>>,
  ) -> Result<&mut Self, String> {
    let mut edits = edits.into_iter().collect::<Vec<_>>();
    // Stable sorting preserves caller order for multiple insertions at one
    // source offset. Sorting by end puts an insertion before a replacement
    // beginning at the same offset, which gives intuitive "insert before"
    // semantics and keeps the insertion outside the replaced range.
    edits.sort_by_key(|edit| (edit.start, edit.end));

    let source = self.source();
    let mut previous_end = 0usize;
    for (index, edit) in edits.iter().enumerate() {
      if edit.start > edit.end {
        return Err(format!(
          "end must be greater than or equal to start, got start: {}, end: {}",
          edit.start, edit.end
        ));
      }
      if edit.end > source.len() {
        return Err(format!(
          "edit range {}..{} exceeds source length {}",
          edit.start,
          edit.end,
          source.len()
        ));
      }
      if !source.is_char_boundary(edit.start) || !source.is_char_boundary(edit.end) {
        return Err(format!(
          "edit range {}..{} is not on UTF-8 character boundaries",
          edit.start, edit.end
        ));
      }
      if index != 0 && edit.start < previous_end {
        return Err(format!(
          "overlapping edit range {}..{} follows an edit ending at {previous_end}",
          edit.start, edit.end
        ));
      }
      previous_end = edit.end;
    }

    let mut staged = self.clone();
    for edit in edits {
      let start = u32::try_from(edit.start)
        .map_err(|_| format!("edit start {} exceeds the 4GB source limit", edit.start))?;
      if edit.start == edit.end {
        let _ = staged.append_left(start, edit.content)?;
        continue;
      }
      let end = u32::try_from(edit.end)
        .map_err(|_| format!("edit end {} exceeds the 4GB source limit", edit.end))?;
      let _ = staged.update_with(start, end, edit.content, edit.options)?;
    }
    *self = staged;
    Ok(self)
  }

  // --- private

  #[expect(
    clippy::needless_pass_by_value,
    reason = "the small options value is consumed only through copyable policy fields"
  )]
  pub(super) fn inner_update_with(
    &mut self,
    start: u32,
    end: u32,
    content: CowStr<'text>,
    opts: UpdateOptions,
    error_if_start_equal_end: bool,
  ) -> Result<&mut Self, String> {
    if error_if_start_equal_end && start == end {
      return Err(
        "Cannot overwrite a zero-length range – use appendLeft or prependRight instead".to_string(),
      );
    }
    if start >= end {
      return Err(format!("end must be greater than start, got start: {start}, end: {end}"));
    }
    self.split_at(start)?;
    self.split_at(end)?;

    // Record the *whole* replaced range, not the start chunk's span: `start..end` may cross a
    // boundary left by an earlier split, in which case the start chunk only covers part of it.
    // Matches magic-string, which stores `original.slice(start, end)` here.
    if opts.keep_original {
      self.store_name(start, end);
    }

    let start_idx = self
      .chunk_by_start
      .get(&start)
      .copied()
      .ok_or_else(|| format!("missing chunk boundary at update start {start}"))?;
    let end_idx = self
      .chunk_by_end
      .get(&end)
      .copied()
      .ok_or_else(|| format!("missing chunk boundary at update end {end}"))?;

    if start_idx != end_idx {
      // When the update range spans multiple chunks, we need to:
      // 1. Detect if any chunk within the range has been moved (via `move()`). A moved
      //    chunk's linked-list successor (`chunk.next`) will differ from its positional
      //    successor (`chunk_by_start[chunk.end]`). Overwriting across such a boundary
      //    is invalid because the chunks are no longer contiguous in the output.
      // 2. Clear each interior/end chunk's content (set to "").
      //
      // This mirrors the JS magic-string `update()` implementation:
      //   https://github.com/Rich-Harris/magic-string/blob/410fd4d/src/MagicString.js#L420-L428
      let mut chunk_idx = start_idx;
      loop {
        let next_in_list = self.chunks[chunk_idx].next;
        let chunk_end = self.chunks[chunk_idx].end();
        let next_by_position = self.chunk_by_start.get(&chunk_end).copied();

        if next_in_list != next_by_position {
          return Err("Cannot overwrite across a split point".to_string());
        }

        // Both are `None` when the walk runs off the end of the list without reaching
        // `end_idx`, i.e. the range is not contiguous in the output — same failure as above.
        let Some(next_idx) = next_in_list else {
          return Err("Cannot overwrite across a split point".to_string());
        };
        chunk_idx = next_idx;
        // Interior chunks always clear intro/outro (`Default` has `overwrite: true`),
        // matching JS magic-string where `chunk.edit('', false)` passes
        // `contentOnly=undefined` (falsy), so intro/outro are always cleared.
        self.chunks[chunk_idx].edit("".into(), EditOptions::default());

        if chunk_idx == end_idx {
          break;
        }
      }
    }

    // Edit the start chunk last — only this chunk receives the replacement content
    // and respects the caller's `overwrite` option (JS `contentOnly`).
    self.chunks[start_idx]
      .edit(content, EditOptions { overwrite: opts.overwrite, store_name: opts.keep_original });
    Ok(self)
  }
}
