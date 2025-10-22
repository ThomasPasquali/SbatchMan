use hashlink::LinkedHashMap;
use saphyr::Yaml;

use crate::core::parsers::ParserError;

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

pub fn lookup_str(yaml: &Yaml, key: &str) -> Result<String, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(y) => match y.as_str() {
      Some(s) => Ok(s.to_string()),
      None => Err(ParserError::WrongType(format!("{:?}", yaml), "string".to_string())),
    },
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}

pub fn lookup_sequence<'a>(yaml: &Yaml<'a>, key: &str) -> Result<Vec<Yaml<'a>>, ParserError> {
  match yaml_lookup(yaml, key) {
    Some(yaml) => match yaml {
      Yaml::Sequence(seq) => Ok(seq.clone()),
      _ => Err(ParserError::WrongType(format!("{:?}", yaml), "sequence".to_string())),
    },
    None => Err(ParserError::MissingKey(key.to_string())),
  }
}