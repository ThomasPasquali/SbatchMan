use std::{collections::HashSet, fs, path::Path};

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
/// Example: "example"
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
/// Example: [item1, item2, item3]
pub fn to_sequence<'a>(yaml: &'a YamlOwned) -> Result<&'a Vec<YamlOwned>, ParserError> {
  match yaml {
    YamlOwned::Sequence(seq) => Ok(seq),
    _ => Err(ParserError::WrongType(
      format!("{:?}", yaml),
      "sequence".to_string(),
    )),
  }
}

/// Convert YAML node to mapping
/// Example: {key1: value1, key2: value2}
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
/// Example: key: "example", given key, returns "example"
pub fn lookup_str(yaml: &YamlOwned, key: &str) -> Result<String, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(value) => to_string(value),
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}

/// Lookup mapping by key and return sequence
/// Example: key: [item1, item2, item3], given key, returns the sequence
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
/// Example: key: {key1: value1, key2: value2}, given key, returns the mapping
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

/// Check that all keys in the mapping are among the valid keys. Returns an error if an invalid key is found.
pub fn check_mapping_keys(
  mapping: &YamlOwned,
  required_keys: &[&str],
  optional_keys: &[&str],
) -> Result<(), ParserError> {
  let mut seen_keys = HashSet::new();
  let valid_keys = [required_keys, optional_keys].concat();

  for key in to_mapping(mapping)?.keys() {
    let key_str = to_string(key)?;
    if !valid_keys.contains(&key_str.as_str()) {
      return Err(ParserError::InvalidKey(
        key_str,
        valid_keys.join(", "),
      ));
    }
    if seen_keys.contains(&key_str) {
      return Err(ParserError::InvalidKey(
        format!("Duplicate key found: {}", key_str),
        valid_keys.join(", "),
      ));
    }
    seen_keys.insert(key_str);
  }

  for key in required_keys {
    if !seen_keys.contains(*key) {
      return Err(ParserError::MissingKey(key.to_string()));
    }
  }

  Ok(())
}