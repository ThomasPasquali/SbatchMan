use std::collections::HashMap;

use crate::core::parsers::template_parser::Template;
use crate::core::parsers::utils::value_from_str;
use crate::core::parsers::{ParserError, utils::to_string};
use hashlink::LinkedHashMap;
use saphyr::{ScalarOwned as YamlOwnedScalar, Tag, YamlOwned};

#[derive(Debug, Clone)]
pub(crate) enum Scalar {
  String(String),
  Int(i64),
  Float(f64),
  Bool(bool),
}

impl Scalar {
  pub fn to_string(&self) -> String {
    match self {
      Scalar::String(s) => s.clone(),
      Scalar::Int(i) => i.to_string(),
      Scalar::Float(f) => f.to_string(),
      Scalar::Bool(b) => b.to_string(),
    }
  }
}

#[derive(Debug)]
pub struct ListVar {
  items: Vec<Scalar>,
}

impl ListVar {
  pub fn get(&self, index: usize) -> Result<Scalar, ParserError> {
    self.items.get(index).ok_or_else(|| {
      ParserError::EvalError(format!("Index {} out of bounds for list of length {}", index, self.items.len()))
    }).cloned()
  }
}

#[derive(Debug, PartialEq)]
pub enum MapKind {
  Cluster,
  Standard,
}

#[derive(Debug)]
pub struct MapVar {
  map: HashMap<String, BasicVar>,
  kind: MapKind,
}

impl MapVar {
  pub fn get(&self, key: &str) -> Result<&BasicVar, ParserError> {
    self.map.get(key).ok_or_else(|| {
      ParserError::EvalError(format!("Key '{}' not found in map variable", key))
    })
  }

  pub fn kind(&self) -> &MapKind {
    &self.kind
  }
}

#[derive(Debug)]
pub struct PythonVar {
  pub template: Template,
}

#[derive(Debug)]
pub(crate) enum BasicVar {
  Scalar(Scalar),
  List(ListVar),
}
impl From<ListVar> for BasicVar {
  fn from(v: ListVar) -> Self {
    BasicVar::List(v)
  }
}

#[derive(Debug)]
pub(crate) enum CompleteVar {
  BasicVar(BasicVar),
  Map(MapVar),
  Python(PythonVar),
}

impl From<ListVar> for CompleteVar {
  fn from(v: ListVar) -> Self {
    CompleteVar::BasicVar(BasicVar::List(v))
  }
}
impl From<MapVar> for CompleteVar {
  fn from(v: MapVar) -> Self {
    CompleteVar::Map(v)
  }
}
impl From<PythonVar> for CompleteVar {
  fn from(v: PythonVar) -> Self {
    CompleteVar::Python(v)
  }
}

impl CompleteVar {
  pub fn as_list(&self) -> Option<&ListVar> {
    match self {
      CompleteVar::BasicVar(BasicVar::List(list)) => Some(list),
      _ => None,
    }
  }

  pub fn as_map(&self) -> Option<&MapVar> {
    match self {
      CompleteVar::Map(map) => Some(map),
      _ => None,
    }
  }

  pub fn as_scalar(&self) -> Option<&Scalar> {
    match self {
      CompleteVar::BasicVar(BasicVar::Scalar(scalar)) => Some(scalar),
      _ => None,
    }
  }
}

/// Helper macro to create WrongType ParserError
macro_rules! wrong_type_err {
  ($value:expr, $expected:expr) => {
    ParserError::WrongType(format!("{:?}", $value), $expected.to_string())
  };
}

/// Parse a scalar YAML node into Scalar enum.
fn parse_scalar(s: &YamlOwnedScalar) -> Result<Scalar, ParserError> {
  match s {
    YamlOwnedScalar::String(s) => Ok(Scalar::String(s.to_string())),
    YamlOwnedScalar::Integer(i) => Ok(Scalar::Int(*i)),
    YamlOwnedScalar::FloatingPoint(f) => Ok(Scalar::Float(**f)),
    YamlOwnedScalar::Boolean(b) => Ok(Scalar::Bool(*b)),
    _ => {
      return Err(wrong_type_err!(s, "string, integer, float, or boolean"));
    }
  }
}

fn parse_tagged_basic_var(tag: &Tag, s: &YamlOwned) -> Result<BasicVar, ParserError> {
  match tag.suffix.as_str() {
    "file" => {
      let path = to_string(s)?;
      let content = std::fs::read_to_string(&path)
        .map_err(|e| ParserError::FileReadError(path.to_string(), e.to_string()))?
        .lines()
        .map(|line| Scalar::String(line.to_string()))
        .collect();
      Ok(ListVar { items: content }.into())
    }
    "dir" => {
      let path = to_string(s)?;
      let mut file_list: Vec<Scalar> = Vec::new();
      let entries = std::fs::read_dir(&path)
        .map_err(|e| ParserError::FileReadError(path.to_string(), e.to_string()))?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<std::path::PathBuf>, std::io::Error>>()?;

      for entry in entries.iter() {
        if let Some(entry_str) = entry.to_str() {
          file_list.push(Scalar::String(entry_str.to_string()));
        }
      }
      Ok(ListVar { items: file_list }.into())
    }
    _ => {
      return Err(wrong_type_err!(tag, "unknown tag"));
    }
  }
}

/// Parse a tagged YAML node into Scalar enum. Handles !file, !dir, and !python tags.
/// TODO: is working directory correct?
fn parse_tagged(tag: &Tag, s: &YamlOwned) -> Result<CompleteVar, ParserError> {
  match tag.suffix.as_str() {
    "python" => {
      let code = to_string(s)?;
      let template = Template::parse_str(&code)?;
      Ok(PythonVar { template }.into())
    }
    _ => {
      parse_tagged_basic_var(tag, s).map(CompleteVar::BasicVar)
    }
  }
}

/// Parse a sequence of scalars into Vec<Scalar>
fn parse_sequence_of_scalars(seq: &Vec<YamlOwned>) -> Result<Vec<Scalar>, ParserError> {
  let mut scalars: Vec<Scalar> = Vec::new();
  for item in seq.iter() {
    match item {
      YamlOwned::Value(s) => {
        scalars.push(parse_scalar(s)?);
      }
      _ => {
        return Err(wrong_type_err!(item, "scalar"));
      }
    }
  }
  Ok(scalars)
}

/// Parse a mapping of `BasicVar`s into HashMap<String, BasicVar>
fn parse_map_of_basic_vars(
  map: &LinkedHashMap<YamlOwned, YamlOwned>,
) -> Result<HashMap<String, BasicVar>, ParserError> {
  let mut result: HashMap<String, BasicVar> = HashMap::new();

  for (k, v) in map.iter() {
    let key_str = k.as_str().ok_or(wrong_type_err!(k, "string"))?;
    result.insert(key_str.to_string(), parse_basic_var(v)?);
  }

  Ok(result)
}

/// Parse only basic variable (scalar or list). Return error if anything else.
fn parse_basic_var(yaml: &YamlOwned) -> Result<BasicVar, ParserError> {
  match yaml {
    YamlOwned::Value(s) => Ok(BasicVar::Scalar(parse_scalar(s)?)),
    YamlOwned::Tagged(tag, s) => {
      parse_tagged_basic_var(tag, s)
    },
    YamlOwned::Sequence(seq) => Ok(
      ListVar {
        items: parse_sequence_of_scalars(seq)?,
      }
      .into(),
    ),
    _ => {
      return Err(wrong_type_err!(yaml, "scalar or list"));
    }
  }
}

/// Convert &str into Yaml using Yaml::value_from_str
macro_rules! yaml_str {
  ($s:expr) => {
    value_from_str($s)
  };
}

/// Main function to parse variables from a YAML node
pub fn parse_variables(
  yaml: &LinkedHashMap<YamlOwned, YamlOwned>,
) -> Result<HashMap<String, CompleteVar>, ParserError> {
  let mut result: HashMap<String, CompleteVar> = HashMap::new();
  // Ensure the top-level YAML is a mapping
  for (k, v) in yaml.iter() {
    let k = k.as_str().ok_or(wrong_type_err!(k, "string"))?;
    let v = match v {
      YamlOwned::Value(s) => parse_scalar(s).map(|x| CompleteVar::BasicVar(BasicVar::Scalar(x)))?,
      YamlOwned::Tagged(tag, s) => parse_tagged(tag, s)?,
      YamlOwned::Sequence(seq) => {
        parse_sequence_of_scalars(seq).map(|x| ListVar { items: x }.into())?
      }
      YamlOwned::Mapping(map) => {
        // Check for "per_cluster" key to determine if it's a ClusterMap
        if let Some(cluster_map) = map.get(&yaml_str!("per_cluster")) {
          // Parse the "per_cluster" mapping and construct the ClusterMap
          MapVar {
            map: parse_map_of_basic_vars(
              cluster_map
                .as_mapping()
                .ok_or(wrong_type_err!(map, "map"))?,
            )?,
            kind: MapKind::Cluster,
          }
          .into()
        } else if let Some(map) = map.get(&yaml_str!("map")) {
          // Parse as a standard mapping variable
          MapVar {
            map: parse_map_of_basic_vars(map.as_mapping().ok_or(wrong_type_err!(map, "map"))?)?,
            kind: MapKind::Standard,
          }
          .into()
        } else {
          return Err(wrong_type_err!(
            v,
            "mapping with 'per_cluster' or 'map' key"
          ));
        }
      }
      _ => {
        return Err(wrong_type_err!(v, "scalar, list, or mapping"));
      }
    };
    result.insert(k.to_string(), v);
  }
  Ok(result)
}
