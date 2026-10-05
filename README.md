# string-wizard-fast

A focused standalone fork of Rolldown's `string_wizard` crate.

This repository tracks the implementation in `rolldown/rolldown/crates/string_wizard` while keeping the crate useful outside the Rolldown workspace and adding a small transactional edit API for proof-oriented source rewriting.

## Upstream

The initial import is `string_wizard` 1.2.12 from Rolldown commit `45e407b177f5885d04a9795f37f8be71d91f6f17` (`release: v1.2.12`). The original code is MIT licensed; see `LICENSE`.

The standalone `rolldown/string_wizard` repository is an older 0.0.x codebase and is not the source of current crates.io releases.

## Fork changes

The fork deliberately keeps its delta narrow:

- replace `oxc_index::IndexVec` with a tiny local chunk-indexed `Vec`; the crate only used `oxc_index` for chunk IDs/storage, while the Rolldown workspace's published dependency features pulled in Rayon and Serde for standalone consumers;
- add `MagicString::try_new` / `try_with_options` so the 32-bit source-offset limit is checked in release builds instead of only by a debug assertion;
- add `TextEdit` and `MagicString::apply_edits` for atomic batches of non-overlapping edits expressed in ordinary `usize` UTF-8 byte offsets;
- validate complete batches before mutation: range ordering, source bounds, UTF-8 character boundaries, and overlap;
- stage a batch on a clone, so an edit-engine failure leaves the original `MagicString` unchanged.

`apply_edits` intentionally does **not** decide which overlapping semantic rewrite should win and does **not** insert language-specific lexical separators. Those policies belong to the caller.

## Validation

The imported Rolldown tests are retained. Fork-specific tests cover unsorted batches, UTF-8 offsets, overlap rejection, and rollback when a later edit fails after validation.

The intended lightweight consumer configuration is:

```toml
string_wizard = { git = "https://github.com/codeandsolder/string-wizard-fast.git", rev = "<commit>", default-features = false }
```

With default features disabled, the runtime dependency graph contains no `oxc_index`, Rayon, Crossbeam, or Serde.

## Updating from Rolldown

When updating:

1. identify the Rolldown release commit that produced the desired `string_wizard` crate;
2. diff `crates/string_wizard` against the upstream commit recorded here;
3. port upstream source/test changes first;
4. reapply only the small fork delta above;
5. run both default-feature and no-default-feature tests and Clippy on Rust 1.99 or newer;
6. update the upstream commit recorded in this README.

Prefer contributing generally useful correctness fixes upstream. Keep fork-only changes limited to standalone dependency hygiene and transactional source-edit ergonomics.
