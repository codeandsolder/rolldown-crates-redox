use crate::{CowStr, UpdateOptions};

use super::MagicString;

#[derive(Debug)]
pub struct ReplaceOptions {
  /// The maximum number of times to replace the pattern. Default is `1`.
  pub count: usize,
  /// This will store the original content in the `name` field of the generated sourcemap.
  ///
  /// Default is `false`.
  pub store_original_in_sourcemap: bool,
}

impl Default for ReplaceOptions {
  fn default() -> Self {
    Self { count: 1, store_original_in_sourcemap: false }
  }
}

impl<'text> MagicString<'text> {
  /// # Errors
  /// Propagates errors from [`Self::replace_with`].
  pub fn replace(&mut self, from: &str, to: impl Into<CowStr<'text>>) -> Result<&mut Self, String> {
    self.replace_with(from, to, ReplaceOptions::default())
  }

  /// # Errors
  /// Propagates errors from [`Self::replace_with`].
  pub fn replace_all(
    &mut self,
    from: &str,
    to: impl Into<CowStr<'text>>,
  ) -> Result<&mut Self, String> {
    self.replace_with(from, to, ReplaceOptions { count: usize::MAX, ..Default::default() })
  }

  /// # Errors
  /// Returns an error when a matched source range cannot be updated safely.
  #[expect(
    clippy::needless_pass_by_value,
    reason = "the small options value is part of the upstream-compatible public API"
  )]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "match offsets are bounded by MagicString's validated 32-bit source length"
  )]
  pub fn replace_with(
    &mut self,
    from: &str,
    to: impl Into<CowStr<'text>>,
    options: ReplaceOptions,
  ) -> Result<&mut Self, String> {
    let to: CowStr<'text> = to.into();
    let matches = memchr::memmem::find_iter(self.source.as_bytes(), from.as_bytes())
      .take(options.count)
      .map(|start| (start as u32, (start + from.len()) as u32))
      .collect::<Vec<_>>();
    for (match_start, end) in matches {
      let _ = self.update_with(
        match_start,
        end,
        to.clone(),
        UpdateOptions { overwrite: true, keep_original: options.store_original_in_sourcemap },
      )?;
    }

    Ok(self)
  }
}
