//! Bounded dynamic-import expression reduction adapted from Rolldown/Vite.

use std::{error::Error, fmt, path::Path};

use oxc_ast::ast::{Argument, BinaryExpression, CallExpression, Expression, TemplateLiteral};
use oxc_syntax::operator::BinaryOperator;

const IGNORED_PROTOCOLS: [&str; 3] = ["data:", "http:", "https:"];
const SPECIAL_PARAMS: [&str; 4] = ["raw", "sharedworker", "url", "worker"];

/// Why a non-literal dynamic import cannot be reduced to a bounded relative glob.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DynamicImportGlobError {
  /// The source already contains `*`, which would be ambiguous with introduced wildcards.
  LiteralWildcard,
  /// A binary expression used an operator other than string-concatenating `+`.
  UnsupportedOperator(String),
  /// The static part does not prove a relative `./` or `../` import.
  InvalidRelativePrefix,
  /// The static part does not contain a file extension.
  MissingExtension,
}

impl fmt::Display for DynamicImportGlobError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::LiteralWildcard => formatter.write_str("dynamic import contains a literal '*'"),
      Self::UnsupportedOperator(operator) => {
        write!(formatter, "dynamic import operator {operator:?} is not supported")
      }
      Self::InvalidRelativePrefix => {
        formatter.write_str("variable dynamic import must prove a relative './' or '../' prefix")
      }
      Self::MissingExtension => {
        formatter.write_str("variable dynamic import must include a static file extension")
      }
    }
  }
}

impl Error for DynamicImportGlobError {}

/// Return whether a Vite-style query string selects a non-module resource mode.
#[must_use]
pub fn has_special_dynamic_import_query(query: &str) -> bool {
  if query.len() < 2 || !query.starts_with('?') {
    return false;
  }
  query[1..].split('&').any(|param| SPECIAL_PARAMS.contains(&param))
}

/// Reduce a dynamic-import expression to a relative glob when it is bounded enough to match.
///
/// This follows Rolldown/Vite's dynamic-import-vars semantics: template substitutions,
/// unknown expressions and spread arguments become one path-segment wildcard; string `+`
/// and `.concat(...)` are composed recursively. HTTP(S) and data-URL patterns are ignored.
///
/// # Errors
/// Returns [`DynamicImportGlobError`] when the expression contains an ambiguous literal
/// wildcard, uses an unsupported binary operator, or cannot prove a relative import with a
/// static file extension.
pub fn dynamic_import_to_glob(
  expression: &Expression<'_>,
) -> Result<Option<String>, DynamicImportGlobError> {
  let mut glob = expr_to_glob(expression)?;
  while glob.contains("**") {
    glob = glob.replace("**", "*");
  }

  if IGNORED_PROTOCOLS.iter().any(|protocol| glob.starts_with(protocol)) || !glob.contains('*') {
    return Ok(None);
  }
  if let Some(query_start) = glob.find('?')
    && has_special_dynamic_import_query(&glob[query_start..])
  {
    return Ok(None);
  }

  if !glob.starts_with("./") && !glob.starts_with("../") {
    return Err(DynamicImportGlobError::InvalidRelativePrefix);
  }
  if Path::new(&glob).extension().is_none() {
    return Err(DynamicImportGlobError::MissingExtension);
  }

  Ok(Some(escape_glob_literals(&glob)))
}

fn expr_to_glob(expression: &Expression<'_>) -> Result<String, DynamicImportGlobError> {
  match expression {
    Expression::TemplateLiteral(template) => template_literal_to_glob(template),
    Expression::CallExpression(call) => call_expression_to_glob(call),
    Expression::BinaryExpression(binary) => binary_expression_to_glob(binary),
    Expression::StringLiteral(literal) => sanitize_string(&literal.value),
    _ => Ok("*".to_string()),
  }
}

fn template_literal_to_glob(
  template: &TemplateLiteral<'_>,
) -> Result<String, DynamicImportGlobError> {
  let mut glob = String::new();
  for (index, quasi) in template.quasis.iter().enumerate() {
    glob.push_str(&sanitize_string(&quasi.value.raw)?);
    if let Some(expression) = template.expressions.get(index) {
      glob.push_str(&expr_to_glob(expression)?);
    }
  }
  Ok(glob)
}

fn call_expression_to_glob(call: &CallExpression<'_>) -> Result<String, DynamicImportGlobError> {
  let Expression::StaticMemberExpression(member) = &call.callee else {
    return Ok("*".to_string());
  };
  if member.property.name.as_str() != "concat" {
    return Ok("*".to_string());
  }

  let mut glob = expr_to_glob(&member.object)?;
  for argument in &call.arguments {
    match argument {
      Argument::SpreadElement(_) => glob.push('*'),
      _ => glob.push_str(&expr_to_glob(argument.to_expression())?),
    }
  }
  Ok(glob)
}

fn binary_expression_to_glob(
  binary: &BinaryExpression<'_>,
) -> Result<String, DynamicImportGlobError> {
  if binary.operator != BinaryOperator::Addition {
    return Err(DynamicImportGlobError::UnsupportedOperator(binary.operator.as_str().to_string()));
  }
  let mut glob = expr_to_glob(&binary.left)?;
  glob.push_str(&expr_to_glob(&binary.right)?);
  Ok(glob)
}

fn sanitize_string(value: &str) -> Result<String, DynamicImportGlobError> {
  if value.contains('*') {
    return Err(DynamicImportGlobError::LiteralWildcard);
  }
  Ok(value.to_string())
}

fn escape_glob_literals(glob: &str) -> String {
  let mut escaped = String::with_capacity(glob.len());
  for character in glob.chars() {
    match character {
      '?' | '[' | ']' | '{' | '}' => {
        escaped.push('[');
        escaped.push(character);
        escaped.push(']');
      }
      _ => escaped.push(character),
    }
  }
  escaped
}

#[cfg(test)]
mod tests {
  use std::io;

  use oxc_allocator::Allocator;
  use oxc_parser::Parser;
  use oxc_span::SourceType;

  use super::*;

  fn parse_expression<'a>(
    allocator: &'a Allocator,
    source: &'a str,
  ) -> Result<Expression<'a>, io::Error> {
    Parser::new(allocator, source, SourceType::default())
      .parse_expression()
      .map_err(|_| io::Error::other("test expression failed to parse"))
  }

  fn pattern(source: &str) -> Result<Option<String>, Box<dyn Error>> {
    let allocator = Allocator::default();
    let expression = parse_expression(&allocator, source)?;
    Ok(dynamic_import_to_glob(&expression)?)
  }

  #[test]
  fn template_variable_filename() -> Result<(), Box<dyn Error>> {
    assert_eq!(pattern("`./foo/${bar}.js`")?, Some("./foo/*.js".to_string()));
    Ok(())
  }

  #[test]
  fn concat_and_binary_forms() -> Result<(), Box<dyn Error>> {
    assert_eq!(pattern("'./foo/'.concat(bar, '/x.js')")?, Some("./foo/*/x.js".to_string()));
    assert_eq!(pattern("'./foo/' + bar + '.js'")?, Some("./foo/*.js".to_string()));
    Ok(())
  }

  #[test]
  fn repeated_unknowns_collapse() -> Result<(), Box<dyn Error>> {
    assert_eq!(pattern("`./foo/${bar}${baz}/${x}${y}.js`")?, Some("./foo/*/*.js".to_string()));
    Ok(())
  }

  #[test]
  fn external_and_literal_imports_are_not_globs() -> Result<(), Box<dyn Error>> {
    assert_eq!(pattern("`https://example.com/${version}/index.js`")?, None);
    assert_eq!(pattern("'./literal.js'")?, None);
    assert_eq!(pattern("`./chunks/${name}.js?worker`")?, None);
    Ok(())
  }

  #[test]
  fn rejects_ambiguous_or_unbounded_forms() -> Result<(), Box<dyn Error>> {
    let allocator = Allocator::default();
    let star = parse_expression(&allocator, "`./*${name}.js`")?;
    assert_eq!(dynamic_import_to_glob(&star), Err(DynamicImportGlobError::LiteralWildcard));
    let bare = parse_expression(&allocator, "name")?;
    assert_eq!(dynamic_import_to_glob(&bare), Err(DynamicImportGlobError::InvalidRelativePrefix));
    Ok(())
  }

  #[test]
  fn escapes_glob_metacharacters_and_detects_special_queries() -> Result<(), Box<dyn Error>> {
    assert_eq!(pattern("`./${name}/[foo].js`")?, Some("./*/[[]foo[]].js".to_string()));
    assert!(has_special_dynamic_import_query("?worker&inline"));
    assert!(!has_special_dynamic_import_query("?v=123"));
    Ok(())
  }
}
