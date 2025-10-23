use std::{fs, path::Path};

use saphyr::{LoadableYamlNode, ScalarOwned, YamlOwned};

use crate::core::parsers::ParserError;

pub fn value_from_str(s: &str) -> YamlOwned {
  YamlOwned::Value(ScalarOwned::String(s.to_string()))
}

pub fn yaml_lookup<'a>(node: &'a YamlOwned, key: &str) -> Option<&'a YamlOwned> {
  if let YamlOwned::Mapping(map) = node {
    return map.get(&value_from_str(key));
  }
  None
}

pub fn lookup_str(yaml: &YamlOwned, key: &str) -> Result<String, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(y) => match y.as_str() {
      Some(s) => Ok(s.to_string()),
      None => Err(ParserError::WrongType(
        format!("{:?}", yaml),
        "string".to_string(),
      )),
    },
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}

pub fn lookup_sequence<'a>(
  yaml: &'a YamlOwned,
  key: &str,
) -> Result<&'a Vec<YamlOwned>, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(yaml) => match yaml {
      YamlOwned::Sequence(seq) => Ok(seq),
      _ => Err(ParserError::WrongType(
        format!("{:?}", yaml),
        "sequence".to_string(),
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
