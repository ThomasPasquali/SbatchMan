use std::collections::HashMap;

use crate::core::parsers::ParserError;
use crate::core::parsers::utils::value_from_str;
use hashlink::LinkedHashMap;
use saphyr::{ScalarOwned as YamlOwnedScalar, YamlOwned};

#[derive(Debug, PartialEq)]
pub enum Scalar {
  String(String),
  Int(i64),
  Float(f64),
  Bool(bool),
  File(String),
  Directory(String),
}

#[derive(Debug, PartialEq)]
pub enum BasicVar {
  Scalar(Scalar),
  List(Vec<Scalar>),
}

#[derive(Debug, PartialEq)]
pub struct ClusterMap {
  pub default: Option<BasicVar>,
  pub per_cluster: HashMap<String, BasicVar>,
}

#[derive(Debug, PartialEq)]
pub enum CompleteVar {
  Scalar(Scalar),
  List(Vec<Scalar>),
  StandardMap(HashMap<String, BasicVar>),
  ClusterMap(ClusterMap),
}

#[derive(Debug)]
pub struct Variable {
  pub name: String,
  pub contents: CompleteVar,
}

impl PartialEq for Variable {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
  }
}

/// Helper macro to create WrongType ParserError
macro_rules! wrong_type_err {
  ($value:expr, $expected:expr) => {
    ParserError::WrongType(format!("{:?}", $value), $expected.to_string())
  };
}

/// Parse a scalar YAML node into Scalar enum. Includes special handling for @file and @dir prefixes.
fn parse_scalar<'a>(s: &'a YamlOwnedScalar) -> Result<Scalar, ParserError> {
  match s {
    YamlOwnedScalar::String(s) => {
      if s.starts_with("@file ") {
        Ok(Scalar::File(s["@file ".len()..].to_string()))
      } else if s.starts_with("@dir ") {
        Ok(Scalar::Directory(s["@dir ".len()..].to_string()))
      } else {
        Ok(Scalar::String(s.to_string()))
      }
    }
    YamlOwnedScalar::Integer(i) => Ok(Scalar::Int(*i)),
    YamlOwnedScalar::FloatingPoint(f) => Ok(Scalar::Float(**f)),
    YamlOwnedScalar::Boolean(b) => Ok(Scalar::Bool(*b)),
    _ => {
      return Err(wrong_type_err!(s, "string, integer, float, or boolean"));
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

/// Parse a mapping into HashMap<String, BasicVar>
fn parse_mapping(
  map: &LinkedHashMap<YamlOwned, YamlOwned>,
) -> Result<HashMap<String, BasicVar>, ParserError> {
  let mut result: HashMap<String, BasicVar> = HashMap::new();

  for (k, v) in map.iter() {
    let key_str = k.as_str().ok_or(wrong_type_err!(k, "string"))?;
    let basic_var = match v {
      YamlOwned::Value(s) => BasicVar::Scalar(parse_scalar(s)?),
      YamlOwned::Sequence(seq) => BasicVar::List(parse_sequence_of_scalars(seq)?),
      _ => {
        return Err(wrong_type_err!(v, "scalar or list"));
      }
    };
    result.insert(key_str.to_string(), basic_var);
  }

  Ok(result)
}

/// Parse only basic variable (scalar or list). Return error if anything else.
fn parse_basic_var(yaml: &YamlOwned) -> Result<BasicVar, ParserError> {
  match yaml {
    YamlOwned::Value(s) => Ok(BasicVar::Scalar(parse_scalar(s)?)),
    YamlOwned::Sequence(seq) => Ok(BasicVar::List(parse_sequence_of_scalars(seq)?)),
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
pub fn parse_variables(yaml: &YamlOwned) -> Result<HashMap<String, Variable>, ParserError> {
  let mut variables: HashMap<String, Variable> = HashMap::new();
  // Ensure the top-level YAML is a mapping
  let map = yaml.as_mapping().ok_or(wrong_type_err!(yaml, "mapping"))?;

  for (k, v) in map.iter() {
    let k = k.as_str().ok_or(wrong_type_err!(k, "string"))?;
    let v = Variable {
      name: k.to_string(),
      // Determine the type of variable based on the YAML object
      contents: match v {
        YamlOwned::Value(s) => parse_scalar(s).map(CompleteVar::Scalar)?,
        YamlOwned::Sequence(seq) => parse_sequence_of_scalars(seq).map(CompleteVar::List)?,
        YamlOwned::Mapping(map) => {
          // Check for "per_cluster" key to determine if it's a ClusterMap
          if let Some(cluster_map) = map.get(&yaml_str!("per_cluster")) {
            // Look up the "default" key, parse it if found, and handle possible errors
            let default = map
              .get(&yaml_str!("default"))
              .map(parse_basic_var)
              .transpose()?;
            // Parse the "per_cluster" mapping and construct the ClusterMap
            CompleteVar::ClusterMap(ClusterMap {
              default,
              per_cluster: parse_mapping(
                cluster_map
                  .as_mapping()
                  .ok_or(wrong_type_err!(map, "map"))?,
              )?,
            })
          } else if let Some(map) = map.get(&yaml_str!("map")) {
            // Parse as a standard mapping variable
            parse_mapping(map.as_mapping().ok_or(wrong_type_err!(map, "map"))?)
              .map(CompleteVar::StandardMap)?
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
      },
    };
    variables.insert(v.name.clone(), v);
  }
  Ok(variables)
}
