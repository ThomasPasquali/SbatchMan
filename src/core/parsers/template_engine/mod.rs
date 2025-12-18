use crate::core::parsers::{ParserError, multi_hashmap::MultiHashMap, variables::Variable};

/**
 * Simple module that provides efficient variable substitution in strings.
 * Supported syntax:
 * - `${var}`: Replaces with the value of `var`.
 * - `${map}[key]`: Replaces with the value associated with `key` in `map`.
 * - `${map}[${var}]`: Replaces with the value associated with the value of `var` in `map`.
 */

 #[derive(Debug)]
pub struct Template {
  template: Vec<TemplatePart>,
}

#[derive(Debug)]
enum TemplatePart {
  Literal(String),
  Variable(String),
  MapLookup {
    map_name: String,
    key: MapKey,
  },
}

#[derive(Debug)]
enum MapKey {
  Literal(String),
  Variable(String),
}

impl Template {
  fn render(&self, variables: MultiHashMap<String, Variable>) -> Result<String, ParserError> {
    let mut result = String::new();
    for part in &self.template {
      match part {
        TemplatePart::Literal(s) => result.push_str(s),
        TemplatePart::Variable(var_name) => {
          if let Some(value) = variables.get(var_name) {
            result.push_str(&value.to_string());
          } else {
            return Err(ParserError::UndefinedVariable(var_name.clone()));
          }
        }
        TemplatePart::MapLookup { map_name, key } => {
          if let Some(map_value) = variables.get(map_name) {
            // Assuming map_value is a HashMap<String, String> for simplicity
            let map: &HashMap<String, String> = map_value.as_any().downcast_ref().ok_or_else(|| ParserError::WrongType(format!("{:?}", map_value), "HashMap<String, String>".to_string()))?;
            let lookup_key = match key {
              MapKey::Literal(k) => k.clone(),
              MapKey::Variable(k_var) => {
                if let Some(k_value) = variables.get(k_var) {
                  k_value.to_string()
                } else {
                  return Err(ParserError::UndefinedVariable(k_var.clone()));
                }
              }
            };
            if let Some(value) = map.get(&lookup_key) {
              result.push_str(value);
            } else {
              return Err(ParserError::UndefinedMapKey(map_name.clone(), lookup_key));
            }
          } else {
            return Err(ParserError::UndefinedVariable(map_name.clone()));
          }
        }
      }
    }
    Ok(result)
  }
}