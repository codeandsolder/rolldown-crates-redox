use std::borrow::Cow;

use oxc_sourcemap::Token;
use rolldown_redox::sourcemap::{
  SourceJoiner, SourceMap, SourceMapSource, adjust_sourcemap_dst_lines, collapse_sourcemaps,
  empty_sourcemap,
};

fn map(source: &str, content: &str, dst_line: u32, src_line: u32) -> SourceMap {
  SourceMap::new(
    None,
    vec![],
    None,
    vec![Cow::Owned(source.to_string())],
    vec![Some(Cow::Owned(content.to_string()))],
    vec![Token::new(dst_line, 0, src_line, 0, Some(0), None)].into_boxed_slice(),
    None,
  )
}

#[test]
fn collapse_empty_and_single_are_total() {
  let empty = collapse_sourcemaps(&[]);
  assert_eq!(empty.get_tokens().count(), 0);

  let one = map("original.js", "const x = 1;", 0, 0);
  let collapsed = collapse_sourcemaps(&[&one]);
  assert_eq!(collapsed.get_sources().collect::<Vec<_>>(), ["original.js"]);
  assert_eq!(collapsed.get_source_content(0), Some("const x = 1;"));
}

#[test]
fn collapse_traces_through_an_intermediate_map() {
  let original_to_intermediate = map("original.js", "const value = 1;", 0, 3);
  let intermediate_to_output = map("intermediate.js", "generated", 0, 0);

  let collapsed = collapse_sourcemaps(&[&original_to_intermediate, &intermediate_to_output]);
  let token = collapsed.get_token(0).expect("collapsed map should retain its token");

  assert_eq!(collapsed.get_sources().collect::<Vec<_>>(), ["original.js"]);
  assert_eq!(token.get_src_line(), 3);
  assert_eq!(token.get_source_id(), Some(0));
}

#[test]
fn joiner_offsets_owned_maps_without_losing_sources() {
  let mut joiner = SourceJoiner::default();
  joiner.prepend_source("// banner".to_string());
  joiner.append_source(SourceMapSource::new(
    "first();".to_string(),
    map("first.js", "first source", 0, 0),
  ));
  joiner.append_source(SourceMapSource::new(
    "second();".to_string(),
    map("second.js", "second source", 0, 0),
  ));

  let (content, joined) = joiner.join();
  let joined = joined.expect("mapped inputs should produce a map");

  assert_eq!(content, "// banner\nfirst();\nsecond();");
  assert_eq!(joined.get_sources().collect::<Vec<_>>(), ["first.js", "second.js"]);
  assert_eq!(joined.get_token(0).map(|token| token.get_dst_line()), Some(1));
  assert_eq!(joined.get_token(1).map(|token| token.get_dst_line()), Some(2));
}

#[test]
fn destination_line_adjustment_drops_prefix_tokens() {
  let map = SourceMap::new(
    None,
    vec![],
    None,
    vec![Cow::Borrowed("source.js")],
    vec![None],
    vec![Token::new(0, 0, 0, 0, Some(0), None), Token::new(2, 0, 2, 0, Some(0), None)]
      .into_boxed_slice(),
    None,
  );

  let adjusted = adjust_sourcemap_dst_lines(map, 1);
  let tokens = adjusted.get_tokens().collect::<Vec<_>>();
  assert_eq!(tokens.len(), 1);
  assert_eq!(tokens[0].get_dst_line(), 1);
}

#[test]
fn explicit_empty_map_is_empty() {
  let map = empty_sourcemap();
  assert_eq!(map.get_tokens().count(), 0);
  assert_eq!(map.get_sources().count(), 0);
}
