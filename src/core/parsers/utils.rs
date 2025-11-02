use std::{fs, path::Path};

use hashlink::LinkedHashMap;
use saphyr::{LoadableYamlNode, ScalarOwned, YamlOwned};

use crate::core::parsers::ParserError;

/// Convert a string to a YAML node
pub(crate) fn value_from_str(s: &str) -> YamlOwned {
  YamlOwned::Value(ScalarOwned::String(s.to_string()))
}

/// Lookup a YAML mapping by key
pub(crate) fn yaml_lookup<'a>(node: &'a YamlOwned, key: &str) -> Option<&'a YamlOwned> {
  if let YamlOwned::Mapping(map) = node {
    return map.get(&value_from_str(key));
  }
  None
}

/// Convert YAML node to string
pub fn to_string(yaml: &YamlOwned) -> Result<String, ParserError> {
  match yaml.as_str() {
    Some(s) => Ok(s.to_string()),
    None => Err(ParserError::WrongType(
      format!("{:?}", yaml),
      "string".to_string(),
    )),
  }
}

/// Convert YAML node to sequence
pub fn to_sequence<'a>(yaml: &'a YamlOwned) -> Result<&'a Vec<YamlOwned>, ParserError> {
  match yaml {
    YamlOwned::Sequence(seq) => Ok(seq),
    _ => Err(ParserError::WrongType(
      format!("{:?}", yaml),
      "sequence".to_string(),
    )),
  }
}

pub fn to_mapping<'a>(
  yaml: &'a YamlOwned,
) -> Result<&'a LinkedHashMap<YamlOwned, YamlOwned>, ParserError> {
  match yaml {
    YamlOwned::Mapping(map) => Ok(map),
    _ => Err(ParserError::WrongType(
      format!("{:?}", yaml),
      "mapping".to_string(),
    )),
  }
}

/// Lookup mapping by key and return string
pub fn lookup_str(yaml: &YamlOwned, key: &str) -> Result<String, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(value) => to_string(value),
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}

/// Lookup mapping by key and return sequence
pub fn lookup_sequence<'a>(
  yaml: &'a YamlOwned,
  key: &str,
) -> Result<&'a Vec<YamlOwned>, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(yaml) => to_sequence(yaml),
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}

/// Lookup a mapping by key and return a map
pub fn lookup_mapping<'a>(
  yaml: &'a YamlOwned,
  key: &str,
) -> Result<&'a LinkedHashMap<YamlOwned, YamlOwned>, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(yaml) => match yaml {
      YamlOwned::Mapping(map) => Ok(map),
      _ => Err(ParserError::WrongType(
        format!("{:?}", yaml),
        "mapping".to_string(),
      )),
    },
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}

/// Load YAML from a file. Returns the first document in the file.
pub fn load_yaml_from_file(path: &Path) -> Result<YamlOwned, ParserError> {
  let text = fs::read_to_string(path)?;
  let yaml = YamlOwned::load_from_str(&text)
    .map_err(ParserError::YamlParseFailed)?
    .into_iter() // Take the first document
    .next()
    .ok_or(ParserError::YamlEmpty)?;
  Ok(yaml)
}
