//! Small Oxc parsing/ownership layer adapted from Rolldown's `EcmaAst` design.
//!
//! The borrowed helpers centralize parser acceptance policy for short-lived AST
//! consumers. [`OwnedProgram`] keeps source text, allocator, and AST together
//! so a parsed program can safely cross function and stage boundaries.

use std::{error::Error, fmt, sync::Arc};

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::Parser;
use oxc_span::SourceType;
use self_cell::self_cell;

/// Result of a borrowed parse where callers may need parser health metadata.
#[derive(Debug)]
pub struct ParseOutcome<'a> {
  /// Parsed program, including when Oxc emitted recoverable diagnostics.
  pub program: Program<'a>,
  /// Number of parser diagnostics emitted.
  pub diagnostics: usize,
  /// Whether Oxc reported an internal parser panic.
  pub panicked: bool,
}

impl ParseOutcome<'_> {
  /// Returns whether this parse is accepted by the strict parser policy.
  #[must_use]
  pub const fn is_clean(&self) -> bool {
    !self.panicked && self.diagnostics == 0
  }
}

/// Parse a program while preserving the parser-health information used by analysis tools.
#[must_use]
pub fn parse_program<'a>(
  allocator: &'a Allocator,
  source: &'a str,
  source_type: SourceType,
) -> ParseOutcome<'a> {
  let parsed = Parser::new(allocator, source, source_type).parse();
  ParseOutcome {
    program: parsed.program,
    diagnostics: parsed.diagnostics.len(),
    panicked: parsed.panicked,
  }
}

/// Parse a program only when Oxc reports neither diagnostics nor a panic.
#[must_use]
pub fn parse_program_strict<'a>(
  allocator: &'a Allocator,
  source: &'a str,
  source_type: SourceType,
) -> Option<Program<'a>> {
  let parsed = parse_program(allocator, source, source_type);
  parsed.is_clean().then_some(parsed.program)
}

/// Compact error returned when an owned parse does not satisfy strict parser policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseFailure {
  diagnostics: usize,
  panicked: bool,
}

impl ParseFailure {
  /// Number of parser diagnostics emitted.
  #[must_use]
  pub const fn diagnostics(self) -> usize {
    self.diagnostics
  }

  /// Whether Oxc reported an internal parser panic.
  #[must_use]
  pub const fn panicked(self) -> bool {
    self.panicked
  }
}

impl fmt::Display for ParseFailure {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "ECMAScript parse rejected: {} diagnostic(s), panicked={}",
      self.diagnostics, self.panicked
    )
  }
}

impl Error for ParseFailure {}

struct ProgramOwner {
  source: Arc<str>,
  allocator: Allocator,
}

struct ProgramDependent<'cell> {
  program: Program<'cell>,
}

self_cell!(
  struct ProgramCell {
    owner: ProgramOwner,

    #[covariant]
    dependent: ProgramDependent,
  }
);

/// Owns source text, Oxc allocator, and the parsed program as one safe value.
pub struct OwnedProgram {
  cell: ProgramCell,
  source_type: SourceType,
}

impl fmt::Debug for OwnedProgram {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("OwnedProgram")
      .field("source_len", &self.source().len())
      .field("source_type", &self.source_type)
      .finish_non_exhaustive()
  }
}

impl OwnedProgram {
  /// Parse and own a program under the strict no-diagnostics/no-panic policy.
  ///
  /// # Errors
  /// Returns [`ParseFailure`] when Oxc emits diagnostics or reports a parser panic.
  pub fn parse(source: impl Into<Arc<str>>, source_type: SourceType) -> Result<Self, ParseFailure> {
    let source = source.into();
    let allocator = Allocator::default();
    let cell = ProgramCell::try_new(ProgramOwner { source, allocator }, |owner| {
      let parsed = parse_program(&owner.allocator, &owner.source, source_type);
      if parsed.is_clean() {
        Ok(ProgramDependent { program: parsed.program })
      } else {
        Err(ParseFailure { diagnostics: parsed.diagnostics, panicked: parsed.panicked })
      }
    })?;
    Ok(Self { cell, source_type })
  }

  /// Original source text backing this AST.
  #[must_use]
  pub fn source(&self) -> &str {
    &self.cell.borrow_owner().source
  }

  /// Oxc source type used for parsing.
  #[must_use]
  pub const fn source_type(&self) -> SourceType {
    self.source_type
  }

  /// Borrow the parsed program.
  #[must_use]
  pub fn program(&self) -> &Program<'_> {
    &self.cell.borrow_dependent().program
  }

  /// Visit the source, allocator, and mutable program together.
  pub fn with_program_mut<Ret>(
    &mut self,
    function: impl for<'inner> FnOnce(&'inner str, &'inner Allocator, &mut Program<'inner>) -> Ret,
  ) -> Ret {
    self.cell.with_dependent_mut(|owner, dependent| {
      function(&owner.source, &owner.allocator, &mut dependent.program)
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn borrowed_helpers_share_clean_parse_policy() {
    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let clean = parse_program(&allocator, "export const answer = 42;", source_type);
    assert!(clean.is_clean());
    assert!(parse_program_strict(&allocator, "export const answer = 42;", source_type).is_some());
  }

  #[test]
  fn owned_program_keeps_source_and_ast_alive_together() -> Result<(), ParseFailure> {
    let mut parsed = OwnedProgram::parse(String::from("const answer = 42;"), SourceType::cjs())?;
    assert_eq!(parsed.source(), "const answer = 42;");
    assert_eq!(parsed.program().body.len(), 1);
    let body_len = parsed.with_program_mut(|source, _allocator, program| {
      assert_eq!(source, "const answer = 42;");
      program.body.len()
    });
    assert_eq!(body_len, 1);
    Ok(())
  }

  #[test]
  fn owned_program_rejects_diagnostics() {
    let failure = OwnedProgram::parse("const =", SourceType::cjs());
    assert!(failure.is_err());
    if let Err(error) = failure {
      assert!(error.panicked() || error.diagnostics() > 0);
    }
  }
}
