# rolldown-crates-redox

Focused, standalone adaptations of useful Rolldown internals for source recovery,
rewriting, deobfuscation, and related tooling.

This is deliberately a small workspace rather than a mirror of Rolldown's many
internal crates:

- `string_wizard`: API-compatible focused fork of Rolldown's current
  `string_wizard`, with lightweight indices and transactional edit batches.
- `rolldown_redox`: closely related reusable ECMAScript/source-map utilities
  that are useful outside a bundler and do not deserve separate repositories. It
  includes source-map chain collapsing/joining plus a feature-gated Oxc parser layer
  that centralizes parse policy and safely owns source + allocator + AST together.

Upstream-derived code retains its original MIT license and attribution. Fork
changes should remain narrow, tested, and useful to standalone consumers.

## Policy

- Rust 1.99 baseline.
- Central `rust-skills2` strict CI gate.
- Workspace lints deny unsafe code, unwrap/expect/panic/todo/unimplemented,
  correctness issues, and high-signal Clippy groups.
- Renovate keeps compatible stable dependencies current after CI while leaving
  pre-1.0 and major updates for review.
