use hashlink::LinkedHashMap;
use log::warn;
use saphyr::{LoadableYamlNode, Yaml};
use std::{fs, path::Path};

use crate::core::parsers::ParserError;

pub fn load_yaml_from_file(path: &Path) -> Result<Yaml<'static>, ParserError> {
  let text = fs::read_to_string(path)?;
  check_and_get_yaml_first_document(
    Yaml::load_from_str(&text)
      .map_err(|e| ParserError::YamlParseError(format!("Failed parsing YAML: {:?}", e)))?,
  )
}

pub fn check_and_get_yaml_first_document(yaml: Vec<Yaml>) -> Result<Yaml, ParserError> {
  if yaml.is_empty() {
    return Err(ParserError::YamlParseError(
      "YAML file is empty or invalid".to_string(),
    ));
  } else if yaml.len() > 1 {
    warn!("YAML file contains more than one document. Considering only the first");
  }
  Ok(yaml.into_iter().next().unwrap())
}

pub fn yaml_lookup<'a, 'b>(node: &'b Yaml<'a>, key: &'b str) -> Option<&'b Yaml<'a>> {
  if let Yaml::Mapping(map) = node {
    return map.get(&Yaml::value_from_str(key));
  }
  None
}

pub fn yaml_lookup_mut<'a, 'b: 'a>(
  node: &'b mut Yaml<'a>,
  key: &'b str,
) -> Option<&'b mut Yaml<'a>> {
  if let Yaml::Mapping(map) = node {
    return map.get_mut(&Yaml::value_from_str(key));
  }
  None
}

pub fn yaml_mapping_merge<'a, 'b, 'c>(original: &'a Yaml<'a>, new: &'b Yaml<'b>) -> Yaml<'c> {
  // Start with a clone of the original
  let mut result = original.clone();

  // Ensure result is a mapping
  if !result.is_mapping() {
    result = Yaml::Mapping(LinkedHashMap::new());
  }

  if let Yaml::Mapping(map_result) = &mut result {
    if let Yaml::Mapping(map_new) = new {
      for (nk, nv) in map_new.iter() {
        if nk.is_string() {
          map_result.insert(Yaml::value_from_str(nk.as_str().unwrap()), nv.clone());
        }
      }
    }
  }

  // Transmute to the output lifetime
  unsafe { std::mem::transmute(result) }
}

pub fn yaml_has_key(node: &Yaml, key: &str) -> bool {
  yaml_lookup(node, key).is_some()
}

pub fn parse_str(yaml: &Yaml, str: &str) -> Result<String, ParserError> {
  match yaml_lookup(yaml, str) {
    Some(y) => match y.as_str() {
      Some(s) => Ok(s.to_string()),
      None => Err(ParserError::WrongType(str.to_string(), "string".to_string())),
    },
    None => Err(ParserError::MissingKey(str.to_string())),
  }
}

pub fn parse_sequence<'a>(yaml: &Yaml<'a>, key: &str) -> Result<Vec<Yaml<'a>>, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(y) => match y {
      Yaml::Sequence(seq) => Ok(seq.clone()),
      _ => Err(ParserError::WrongType(key.to_string(), "sequence".to_string())),
    },
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}