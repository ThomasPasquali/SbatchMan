/// entry_parser.rs
/// Parses configuration entry strings into a structured format to identify the variables used.
/// Strings are parsed against the following syntax:
/// - `{{ var }}`: Standard variable substitution
/// - `{{ map["key"] }}`: Constant map lookup
/// - `{{ map[key] }}` or {{ map.key }}: Dynamic map lookup using another variable as key

use crate::core::parsers::{ParserError, combination_generator::CombinationIterator};

#[derive(Debug)]
pub enum VariableSegment {
  /// Standard variable substitution
  Variable(String),
  /// Constant map lookup with a string key
  ConstantMap {
    map_name: String,
    key: String,
  },
  /// Dynamic map lookup using another variable as key
  VariableMap {
    map_name: String,
    var_name: String,
  },
}

impl VariableSegment {
  pub fn name(&self) -> String {
    match self {
      VariableSegment::Variable(name) => name.clone(),
      VariableSegment::ConstantMap { map_name, key } => format!("{}.\"{}\"", map_name, key),
      VariableSegment::VariableMap { map_name, var_name } => format!("{}.{}", map_name, var_name),
    }
  }
}

#[derive(Debug)]
pub enum EntrySegment {
  Literal(String),
  VariableSegment(VariableSegment),
}
impl From<VariableSegment> for EntrySegment {
  fn from(vs: VariableSegment) -> Self {
    EntrySegment::VariableSegment(vs)
  }
}

#[derive(Debug)]
pub struct ParsedEntry {
  pub segments: Vec<EntrySegment>,
}


impl ParsedEntry {
  pub fn from_str(string: &str) -> Result<Self, ParserError> {
    let mut segments: Vec<EntrySegment> = Vec::new();
    let mut remainder = string;

    while let Some(start) = remainder.find("{{") {
      let end = remainder[start..].find("}}").ok_or_else(|| {
        ParserError::EvalError("Unclosed variable substitution".to_string())
      })? + start;

      // Add literal segment before {{
      if start > 0 {
        segments.push(EntrySegment::Literal(remainder[..start].to_string()));
      }

      // Extract variable expression and convert to lowercase
      let expr = remainder[start + 2..end].trim().to_ascii_lowercase();
      if let Some(bracket_pos) = expr.find('[') {
        // Map lookup
        let map_name = expr[..bracket_pos].trim().to_string();
        let key_expr = expr[bracket_pos + 1..expr.len() - 1].trim();

        if key_expr.starts_with('"') && key_expr.ends_with('"') {
          // Constant map lookup
          let key = key_expr[1..key_expr.len() - 1].to_string();
          segments.push(VariableSegment::ConstantMap { map_name, key }.into());
        } else {
          // Dynamic map lookup
          let var_name = key_expr.to_string();
          segments.push(VariableSegment::VariableMap { map_name, var_name }.into());
        }
      } else if let Some(dot_pos) = expr.find('.') {
        // Dynamic map lookup using dot notation
        let map_name = expr[..dot_pos].trim().to_string();
        let var_name = expr[dot_pos + 1..].trim().to_string();
        segments.push(VariableSegment::VariableMap { map_name, var_name }.into());
      } else {
        // Standard variable
        segments.push(VariableSegment::Variable(expr.to_string()).into());
      }

      remainder = &remainder[end + 2..];
    }

    // Add remaining literal segment
    if !remainder.is_empty() {
      segments.push(EntrySegment::Literal(remainder.to_string()));
    }

    Ok(ParsedEntry { segments })
  }

  /// Returns a string with all variable segments replaced by their evaluated values from the current combination.
  pub fn render(&self, curr_combination: &CombinationIterator) -> Result<String, ParserError> {
    let mut result = String::new();
    for segment in &self.segments {
      match segment {
        EntrySegment::Literal(lit) => result.push_str(lit),
        EntrySegment::VariableSegment(var_seg) => {
          let value = curr_combination.get_segment_value(var_seg)?;
          result.push_str(&value.to_string());
        }
      }
    }
    Ok(result)
  }
}