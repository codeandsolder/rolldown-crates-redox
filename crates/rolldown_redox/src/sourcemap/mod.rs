// Adapted from rolldown/rolldown crates/rolldown_sourcemap (MIT).
mod source;
mod source_joiner;

use std::borrow::Cow;

use oxc_sourcemap::Token;

pub use oxc_sourcemap::{JSONSourceMap, OwnedSourceMap, SourceMapBuilder, SourcemapVisualizer};
pub use source_joiner::SourceJoiner;

/// Rolldown always stores and produces owned sourcemaps, so we alias the
/// lifetime-parameterized `oxc_sourcemap::SourceMap` to its `'static` form.
pub type SourceMap = oxc_sourcemap::SourceMap<'static>;

pub use self::source::{Source, SourceMapSource};

/// Strips the first `lines` destination lines from the sourcemap, decrementing all remaining
/// destination line numbers accordingly.
///
/// This re-anchors a sourcemap after removing a prefix such as a shebang line from generated code.
/// Reuses the map's existing allocations: strings are moved, and the token buffer is mutated
/// in place unless tokens have to be dropped (only when some token maps into the stripped
/// prefix, which rolldown's own maps never do — prepended text carries no tokens).
#[must_use]
pub fn adjust_sourcemap_dst_lines(sourcemap: SourceMap, lines: u32) -> SourceMap {
  if lines == 0 {
    return sourcemap;
  }

  let shift = |token: &Token| {
    Token::new(
      token.get_dst_line() - lines,
      token.get_dst_col(),
      token.get_src_line(),
      token.get_src_col(),
      token.get_source_id(),
      token.get_name_id(),
    )
  };

  let mut parts = sourcemap.into_parts();
  if parts.tokens.iter().any(|t| t.get_dst_line() < lines) {
    parts.tokens = parts.tokens.iter().filter(|t| t.get_dst_line() >= lines).map(shift).collect();
  } else {
    for token in &mut parts.tokens {
      *token = shift(token);
    }
  }
  // The chunk boundaries and VLQ baselines in `token_chunks` describe the pre-shift tokens.
  parts.token_chunks = None;

  SourceMap::from_parts(parts)
}

/// Builds an empty sourcemap with no tokens, sources, names, or contents.
#[must_use]
pub fn empty_sourcemap() -> SourceMap {
  SourceMap::new(None, vec![], None, vec![], vec![], Box::new([]), None)
}

// <https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts>
//
// Input maps may borrow their strings (e.g. a codegen map borrowing the source it was
// printed from) — the output is always owned, since it copies every string it keeps.
// This lets callers collapse a freshly generated map without `into_owned`-ing it first.
#[must_use]
pub fn collapse_sourcemaps(sourcemap_chain: &[&oxc_sourcemap::SourceMap<'_>]) -> SourceMap {
  let Some(first_map) = sourcemap_chain.first() else {
    return empty_sourcemap();
  };
  if let [only] = sourcemap_chain {
    return (*only).clone().into_owned();
  }
  let Some(last_map) = sourcemap_chain.last() else {
    return empty_sourcemap();
  };
  let chain_without_last = &sourcemap_chain[..sourcemap_chain.len() - 1];

  // Concatenate each map's names into one pool, in chain order. A token's merged name_id is then
  // `local_id + offset`, where `offset` is the pool index of that map's first name.
  let mut merged_names: Vec<Cow<'static, str>> = Vec::new();
  let mut append_names = |map: &oxc_sourcemap::SourceMap<'_>| {
    #[expect(
      clippy::cast_possible_truncation,
      reason = "source-map name tables are indexed by u32 and cannot contain more names than the format encodes"
    )]
    let offset = merged_names.len() as u32;
    merged_names.extend(map.get_names().map(|n| Cow::Owned(n.to_owned())));
    offset
  };

  // Pre-compute lookup tables paired with their offsets in reverse order so we avoid reversing
  // on every token lookup.
  let mut chain_with_offsets: Vec<_> = chain_without_last
    .iter()
    .map(|sourcemap| (*sourcemap, sourcemap.generate_lookup_table(), append_names(sourcemap)))
    .collect();
  chain_with_offsets.reverse();
  let last_offset = append_names(last_map);

  // `last_offset` counts the names of every map before the last one. When it is 0, no traced
  // token can carry a name, so the remap loop skips the per-step name tracking.
  let tokens = if last_offset == 0 {
    remap_tokens::<false>(last_map, &chain_with_offsets, last_offset)
  } else {
    remap_tokens::<true>(last_map, &chain_with_offsets, last_offset)
  };

  SourceMap::new(
    None,
    merged_names,
    None,
    first_map.get_sources().map(|s| Cow::Owned(s.to_owned())).collect(),
    first_map.get_source_contents().map(|x| x.map(|s| Cow::Owned(s.to_owned()))).collect(),
    tokens,
    None,
  )
}

/// Remaps `last_map`'s tokens through `chain`, the earlier maps with the nearest one first.
/// `TRACK_NAMES` is a const so that a chain without names compiles without the name work.
fn remap_tokens<const TRACK_NAMES: bool>(
  last_map: &oxc_sourcemap::SourceMap<'_>,
  chain: &[(&oxc_sourcemap::SourceMap<'_>, Vec<&[Token]>, u32)],
  last_offset: u32,
) -> Box<[Token]> {
  last_map
    .get_source_view_tokens()
    .filter_map(|token| {
      let unmapped_token =
        || Token::new(token.get_dst_line(), token.get_dst_col(), 0, 0, None, None);
      if token.get_source_id().is_none() {
        return Some(unmapped_token());
      }

      let mut original_token = token;
      let mut name_id = token.get_name_id().map(|id| id + last_offset);
      for (sourcemap, lookup_table, offset) in chain {
        let traced = sourcemap.lookup_source_view_token_approx(
          lookup_table,
          original_token.get_src_line(),
          original_token.get_src_col(),
        )?;
        if traced.get_source_id().is_none() {
          return Some(unmapped_token());
        }
        if TRACK_NAMES {
          // Prefer the name from this (earlier) map; otherwise carry forward the downstream one.
          name_id = traced.get_name_id().map(|id| id + offset).or(name_id);
        }
        original_token = traced;
      }

      Some(Token::new(
        token.get_dst_line(),
        token.get_dst_col(),
        original_token.get_src_line(),
        original_token.get_src_col(),
        original_token.get_source_id(),
        name_id,
      ))
    })
    .collect()
}
