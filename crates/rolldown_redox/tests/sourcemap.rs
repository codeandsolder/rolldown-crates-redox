#![cfg(feature = "sourcemap")]

use std::borrow::Cow;

use oxc_sourcemap::Token;
use rolldown_redox::sourcemap::{
  SourceJoiner, SourceMap, SourceMapSource, adjust_sourcemap_dst_lines, collapse_sourcemaps,
  compose_sourcemap_source, empty_sourcemap,
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
fn collapse_traces_through_an_intermediate_map() -> Result<(), &'static str> {
  let original_to_intermediate = map("original.js", "const value = 1;", 0, 3);
  let intermediate_to_output = map("intermediate.js", "generated", 0, 0);

  let collapsed = collapse_sourcemaps(&[&original_to_intermediate, &intermediate_to_output]);
  let token = collapsed.get_token(0).ok_or("collapsed map lost its token")?;

  assert_eq!(collapsed.get_sources().collect::<Vec<_>>(), ["original.js"]);
  assert_eq!(token.get_src_line(), 3);
  assert_eq!(token.get_source_id(), Some(0));
  Ok(())
}

#[test]
fn compose_one_source_preserves_sibling_sources() {
  let downstream = SourceMap::new(
    Some(Cow::Borrowed("bundle.js")),
    vec![Cow::Borrowed("bundle_name")],
    None,
    vec![Cow::Borrowed("left.js"), Cow::Borrowed("middle.js"), Cow::Borrowed("right.js")],
    vec![Some(Cow::Borrowed("left")), Some(Cow::Borrowed("middle")), Some(Cow::Borrowed("right"))],
    vec![
      Token::new(0, 0, 1, 0, Some(0), None),
      Token::new(1, 0, 4, 0, Some(1), Some(0)),
      Token::new(2, 0, 8, 0, Some(2), None),
    ]
    .into_boxed_slice(),
    None,
  );
  let upstream = SourceMap::new(
    None,
    vec![Cow::Borrowed("original_name")],
    None,
    vec![Cow::Borrowed("middle-a.ts"), Cow::Borrowed("middle-b.ts")],
    vec![Some(Cow::Borrowed("a")), Some(Cow::Borrowed("b"))],
    vec![Token::new(4, 0, 40, 2, Some(1), Some(0))].into_boxed_slice(),
    None,
  );

  let composed = compose_sourcemap_source(&downstream, 1, &upstream);
  assert_eq!(
    composed.get_sources().collect::<Vec<_>>(),
    ["left.js", "middle-a.ts", "middle-b.ts", "right.js"]
  );
  let tokens = composed.get_tokens().collect::<Vec<_>>();
  assert_eq!(tokens[0].get_source_id(), Some(0));
  assert_eq!(tokens[1].get_source_id(), Some(2));
  assert_eq!(tokens[1].get_src_line(), 40);
  assert_eq!(tokens[1].get_src_col(), 2);
  assert_eq!(tokens[1].get_name_id(), Some(1));
  assert_eq!(tokens[2].get_source_id(), Some(3));
  assert_eq!(composed.get_name(1), Some("original_name"));
  assert_eq!(composed.get_file(), Some("bundle.js"));
}

#[test]
fn compose_one_source_is_total_for_invalid_or_empty_upstream() {
  let downstream = map("only.js", "source", 0, 0);
  let empty = empty_sourcemap();
  assert_eq!(
    compose_sourcemap_source(&downstream, 5, &downstream).to_json_string(),
    downstream.to_json_string()
  );
  assert_eq!(
    compose_sourcemap_source(&downstream, 0, &empty).to_json_string(),
    downstream.to_json_string()
  );
}

#[test]
fn joiner_offsets_owned_maps_without_losing_sources() -> Result<(), &'static str> {
  let mut source_joiner = SourceJoiner::default();
  source_joiner.prepend_source("// banner".to_string());
  source_joiner.append_source(SourceMapSource::new(
    "first();".to_string(),
    map("first.js", "first source", 0, 0),
  ));
  source_joiner.append_source(SourceMapSource::new(
    "second();".to_string(),
    map("second.js", "second source", 0, 0),
  ));

  let (content, joined_map) = source_joiner.join();
  let joined_map = joined_map.ok_or("mapped inputs produced no source map")?;

  assert_eq!(content, "// banner\nfirst();\nsecond();");
  assert_eq!(joined_map.get_sources().collect::<Vec<_>>(), ["first.js", "second.js"]);
  assert_eq!(joined_map.get_token(0).map(|token| token.get_dst_line()), Some(1));
  assert_eq!(joined_map.get_token(1).map(|token| token.get_dst_line()), Some(2));
  Ok(())
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
