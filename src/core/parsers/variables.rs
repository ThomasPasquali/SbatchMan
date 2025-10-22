use std::collections::HashMap;

use hashlink::LinkedHashMap;
use saphyr::{Yaml, Scalar as YamlScalar};

use crate::core::parsers::ParserError;

enum Scalar {
  String(String),
  Int(i64),
  Float(f64),
  Bool(bool),
  File(String),
  Directory(String),
}

enum BasicVar {
  Scalar(Scalar),
  List(Vec<Scalar>),
}

struct ClusterMap {
  default: Option<BasicVar>,
  per_cluster: HashMap<String, BasicVar>,
}

enum CompleteVar {
  Scalar(Scalar),
  List(Vec<Scalar>),
  StandardMap(HashMap<String, BasicVar>),
  ClusterMap(ClusterMap),
}

struct Variable {
  name: String,
  contents: CompleteVar,
}

impl PartialEq for Variable {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
  }
}

fn parse_scalar<'a>(s: &'a YamlScalar) -> Result<Scalar, ParserError<'a>> {
  match s {
    YamlScalar::String(s) => {
      if s.starts_with("@file ") {
        Ok(Scalar::File(s["@file ".len()..].to_string()))
      } else if s.starts_with("@dir ") {
        Ok(Scalar::Directory(s["@dir ".len()..].to_string()))
      } else {
        Ok(Scalar::String(s.to_string()))
      }
    },
    YamlScalar::Integer(i) => Ok(Scalar::Int(*i)),
    YamlScalar::FloatingPoint(f) => Ok(Scalar::Float(**f)),
    YamlScalar::Boolean(b) => Ok(Scalar::Bool(*b)),
    _ => {
      return Err(ParserError::WrongType(s, "string, integer, float, or boolean"));
    }
  }
}

fn parse_sequence_of_scalars<'a>(seq: &'a Vec<Yaml<'a>>) -> Result<Vec<Scalar>, ParserError<'a>> {
  let mut scalars: Vec<Scalar> = Vec::new();
  for item in seq.iter() {
    match item {
      Yaml::Value(s) => {
        scalars.push(parse_scalar(s)?);
      },
      _ => {
        return Err(ParserError::WrongType(item, "scalar"));
      }
    }
  }
  Ok(scalars)
}

fn parse_mapping<'a>(
  map: &'a LinkedHashMap<Yaml<'a>, Yaml<'a>>,
) -> Result<HashMap<String, BasicVar>, ParserError<'a>> {
  let mut result: HashMap<String, BasicVar> = HashMap::new();

  for (k, v) in map.iter() {
    let key_str = k.as_str().ok_or(ParserError::WrongType(k, "string"))?;
    let basic_var = match v {
      Yaml::Value(s) => BasicVar::Scalar(parse_scalar(s)?),
      Yaml::Sequence(seq) => BasicVar::List(parse_sequence_of_scalars(seq)?),
      _ => {
        return Err(ParserError::WrongType(v, "scalar or list"));
      }
    };
    result.insert(key_str.to_string(), basic_var);
  }

  Ok(result)
}

fn parse_basic_var<'a>(yaml: &'a Yaml) -> Result<BasicVar, ParserError<'a>> {
  match yaml {
    Yaml::Value(s) => Ok(BasicVar::Scalar(parse_scalar(s)?)),
    Yaml::Sequence(seq) => Ok(BasicVar::List(parse_sequence_of_scalars(seq)?)),
    _ => {
      return Err(ParserError::WrongType(yaml, "scalar or list"));
    }
  }
}

// Convert &str into Yaml using Yaml::value_from_str
macro_rules! yaml_str {
  ($s:expr) => {
    Yaml::value_from_str($s)
  };
}

fn parse_variables<'a>(
  yaml: &'a Yaml,
) -> Result<HashMap<String, Variable>, ParserError<'a>> {
  let mut variables: HashMap<String, Variable> = HashMap::new();
  let map = yaml.as_mapping().ok_or(ParserError::WrongType(yaml, "mapping"))?;

  for (k, v) in map.iter() {
    let k = k.as_str().ok_or(ParserError::WrongType(k, "string"))?;
    let v = Variable {
      name: k.to_string(),
      contents: match v {
        Yaml::Value(s) => {
          parse_scalar(s).map(CompleteVar::Scalar)?
        },
        Yaml::Sequence(seq) => {
          parse_sequence_of_scalars(seq).map(CompleteVar::List)?
        },
        Yaml::Mapping(map) => {
          if let Some(cluster_map) = map.get(&yaml_str!("per_cluster")) {
            let default = map.get(&yaml_str!("default")).and_then(|d| Some(parse_basic_var(d))).transpose()?;
            CompleteVar::ClusterMap(ClusterMap {
              default,
              per_cluster: parse_mapping(cluster_map.as_mapping().ok_or(ParserError::WrongType(map, "map"))?)?,
            })
          } else if let Some(map) = map.get(&yaml_str!("map")) {
            parse_mapping(map.as_mapping().ok_or(ParserError::WrongType(map, "map"))?).map(CompleteVar::StandardMap)?
          } else {
            return Err(ParserError::WrongType(v, "mapping with 'per_cluster' or 'map' key"));
          }
        }
        _ => {
          return Err(ParserError::WrongType(v, "scalar, list, or mapping"));
        }
      }
    };
    variables.insert(v.name.clone(), v);
  }
  Ok(variables)
}
