use std::{collections::HashMap, hash::Hash};

use saphyr::Yaml;

enum Literal {
  String(String),
  Int(i64),
  Float(f64),
  Bool(bool),
}

enum BasicVar {
  Literal(Literal),
  List(Vec<Literal>),
  File(String),
  Directory(String),
}

enum MappingVar {
  StandardMap(std::collections::HashMap<String, BasicVar>),
  ClusterMap(std::collections::HashMap<String, BasicVar>)
}

enum CompleteVar {
  Literal(Literal),
  List(Vec<Literal>),
  File(String),
  Directory(String),
  Mapping(MappingVar),
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

impl Hash for Variable {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.name.hash(state);
  }
}

struct StringVar {
  value: String, // TODO dynamic String Int Float Bool
}

fn parse_variables(yaml: Yaml) -> Result<HashMap<String, Variable>, crate::core::parsers::ParserError> {
  let mut variables: HashMap<String, Variable> = HashMap::new();

  if let Some(vars_yaml) = crate::core::parsers::utils::yaml_lookup(&yaml, "variables") {
    if let Yaml::Mapping(map) = vars_yaml {
      for (k, v) in map.iter() {
        if let Some(var_name) = k.as_str() {
          let variable = match v {
            Yaml::String(s) => Variable {
              name: var_name.to_string(),
              contents: CompleteVar::String(StringVar { value: s.to_string() }),
            },
            Yaml::Sequence(seq) => {
              let mut values: Vec<String> = Vec::new();
              for item in seq.iter() {
                if let Some(item_str) = item.as_str() {
                  values.push(item_str.to_string());
                } else {
                  return Err(crate::core::parsers::ParserError::WrongType(
                    var_name.to_string(),
                    "string".to_string(),
                  ));
                }
              }
              Variable {
                name: var_name.to_string(),
                contents: CompleteVar::List(ListVar { values }),
              }
            }
            Yaml::Mapping(map) => {
              let mut mapping_values: std::collections::HashMap<String, BasicVar> =
                std::collections::HashMap::new();
              let mut cluster_map = false;
              for (mk, mv) in map.iter() {
                if let Some(mk_str) = mk.as_str() {
                  match mv {
                    Yaml::String(s) => {
                      mapping_values.insert(
                        mk_str.to_string(),
                        BasicVar::String(StringVar { value: s.to_string() }),
                      );
                    }
                    Yaml::Sequence(seq) => {
                      let mut list_values: Vec<String> = Vec::new();
                      for item in seq.iter() {
                        if let Some(item_str) = item.as_str() {
                          list_values.push(item_str.to_string());
                        } else {
                          return Err(crate::core::parsers::ParserError::WrongType(
                            mk_str.to_string(),
                            "string".to_string(),
                          ));
                        }
                      }
                      mapping_values.insert(
                        mk_str.to_string(),
                        BasicVar::List(ListVar { values: list_values }),
                      );
                    }
                    _ => {
                      return Err(crate::core::parsers::ParserError::WrongType(
                        mk_str.to_string(),
                        "string or list".to_string(),
                      ));
                    }
                  }
                  if mk_str == "clusters" {
                    cluster_map = true;
                  }
                } else {
                  return Err(crate::core::parsers::ParserError::WrongType(
                    var_name.to_string(),
                    "string".to_string(),
                  ));
                }
              }
              Variable {
                name: var_name.to_string(),
                contents: CompleteVar::Mapping(MappingVar {
                  values: mapping_values,
                  cluster_map,
                }),
              }
            }
            _ => {
              return Err(crate::core::parsers::ParserError::WrongType(
                var_name.to_string(),
                "string, list, or mapping".to_string(),
              ));
            }
          };
          variables.push(variable);
        } else {
          return Err(crate::core::parsers::ParserError::WrongType(
            "variable name".to_string(),
            "string".to_string(),
          ));
        }
      }
    } else {
      return Err(crate::core::parsers::ParserError::WrongType(
        "variables".to_string(),
        "mapping".to_string(),
      ));
    }
  } else {
    warn!("No 'variables' section found in YAML.");
  }
  Ok(variables)
}