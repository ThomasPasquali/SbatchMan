mod utils;

#[cfg(test)]
mod tests;

use log::{debug, warn};
use saphyr::{LoadableYamlNode, Yaml};
use std::borrow::Cow;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;

use crate::core::parsers::utils::{
  check_and_get_yaml_first_document, load_yaml_from_file, yaml_has_key, yaml_lookup,
  yaml_lookup_mut, yaml_mapping_merge,
};
use crate::core::storage::models::{NewCluster, NewConfig};

#[derive(Error, Debug)]
pub enum ParserError {
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("YAML Parse Error: {0}")]
  YamlParseError(String),
  #[error("Eval Error: {0}")]
  EvalError(String),
}

pub struct NewClusterConfig<'a> {
  pub cluster: NewCluster<'a>,
  pub configs: Vec<NewConfig<'a>>,
}

/// Intermediate representation of a parsed configuration (before mapping to DB models)
#[derive(Debug, Clone)]
pub struct ParsedConfig {
  pub name_template: String,
  pub params: serde_json::Value,
  pub extra_headers: Vec<String>,
}

/// Intermediate representation of a parsed cluster
#[derive(Debug, Clone)]
pub struct ParsedCluster {
  pub name: String,
  pub scheduler: Option<String>,
  pub default_params: serde_json::Value,
  pub configs: Vec<ParsedConfig>,
}

fn load_and_merge_includes<'a>(
  mut base: Box<Yaml<'a>>,
  base_dir: &Path,
) -> Result<Box<Yaml<'a>>, ParserError> {
  if !base.is_mapping() {
    return Ok(base);
  }

  if let Some(include_node) = yaml_lookup(&base, "include") {
    let mut include_files: Vec<String> = Vec::new();
    if include_node.is_string() {
      include_files.push(include_node.as_str().unwrap().to_string());
    } else if let Yaml::Sequence(seq) = &include_node {
      for it in seq.iter() {
        if let Some(s) = it.as_str() {
          include_files.push(s.to_string());
        }
      }
    }
    debug!("Files to be included: {:?}", &include_files);

    for inc_file_path in include_files {
      let inc_path = base_dir.join(&inc_file_path);
      debug!("Parsing included file: {}", inc_path.to_str().unwrap());
      let include_yaml = Box::new(load_yaml_from_file(&inc_path)?);
      let inc_base_dir = inc_path
        .parent()
        .map(|p| p.to_owned())
        .unwrap_or_else(|| PathBuf::from("."));
      // FIXME let include_yaml = load_and_merge_includes(include_yaml, &inc_base_dir)?;

      if let Some(inc_vars) = yaml_lookup(&*include_yaml, "variables") {
        if let Some(base_vars) = yaml_lookup(&*base, "variables") {
          // Create a new merged Yaml with lifetime 'a
          let merged: Yaml<'a> = yaml_mapping_merge(base_vars, inc_vars);
          debug!("Var base: {:?}", base_vars);
          debug!("Var incl: {:?}", inc_vars);
          debug!("Var merg: {:?}", merged);

          // Now update base with the merged result
          if let Some(base_vars_mut) = yaml_lookup_mut(&mut base, "variables") {
            *base_vars_mut = merged;
          } else {
            base
              .as_mapping_mut()
              .unwrap()
              .insert(Yaml::scalar_from_string("variables".to_string()), merged);
          }

        }
      }

      // Warn for unused keys
      if let Yaml::Mapping(include_yaml_map) = &*include_yaml {
        for (k_node, _) in include_yaml_map.iter() {
          if let Some(key) = k_node.as_str() {
            if key != "variables" && key != "include" {
              warn!("The `include` keyword imports only `variables` blocks. Ignored key '{}'", key)
            }
          }
        }
      }

    }
  }

  base.as_mapping_mut().unwrap().remove(&Yaml::value_from_str("include"));
  debug!("SETTED TO BASE: {:?}", base.as_mapping().unwrap());

  Ok(base.clone())
}

/// Collect variables from top-level `variables` block.
fn collect_variables(
  root: &Yaml,
  base_dir: &Path,
) -> Result<HashMap<String, serde_json::Value>, ParserError> {
  if let Some(vars_node) = yaml_lookup(root, "variables") {
    let block = parse_variables_block(&vars_node, base_dir)?;
    Ok(block)
  } else {
    Ok(HashMap::new())
  }
}

/// Parse a `variables` block (Yaml node) into a HashMap<String, serde_json::Value>
fn parse_variables_block(
  vars_node: &Yaml,
  base_dir: &Path,
) -> Result<HashMap<String, serde_json::Value>, ParserError> {
  let mut out = HashMap::new();
  if !vars_node.is_mapping() {
    return Err(ParserError::YamlParseError(
      "'variables' must be a mapping".to_string(),
    ));
  }
  if let Yaml::Mapping(mvec) = vars_node {
    for (k_node, v_node) in mvec.iter() {
      let key = k_node
        .as_str()
        .ok_or_else(|| ParserError::YamlParseError("variable name must be string".to_string()))?
        .to_string();
      if v_node.is_string() {
        let s = v_node.as_str().unwrap();
        if s.starts_with("@file") {
          let parts: Vec<&str> = s.splitn(2, ' ').collect();
          let path_part = if parts.len() == 2 {
            parts[1].trim()
          } else {
            ""
          };
          let resolved = base_dir.join(path_part);
          let content = fs::read_to_string(&resolved)?;
          let lines: Vec<String> = content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
          out.insert(key, serde_json::json!(lines));
          continue;
        } else if s.starts_with("@dir") {
          let parts: Vec<&str> = s.splitn(2, ' ').collect();
          let path_part = if parts.len() == 2 {
            parts[1].trim()
          } else {
            ""
          };
          let resolved = base_dir.join(path_part);
          let mut list = Vec::new();
          if resolved.is_dir() {
            for entry in fs::read_dir(&resolved)? {
              let e = entry?;
              let fname = e.file_name().to_string_lossy().to_string();
              if !fname.starts_with('.') {
                list.push(fname);
              }
            }
          }
          out.insert(key, serde_json::json!(list));
          continue;
        } else {
          out.insert(key, serde_json::json!(s));
          continue;
        }
      } else if v_node.is_sequence() {
        let mut arr = Vec::new();
        if let Yaml::Sequence(seq) = v_node {
          for it in seq.iter() {
            if let Some(s) = it.as_str() {
              arr.push(s.to_string());
            } else {
              arr.push(format!("{:?}", it));
            }
          }
        }
        out.insert(key, serde_json::json!(arr));
        continue;
      } else if v_node.is_mapping() {
        let json = yamlnode_to_json(v_node)?;
        out.insert(key, json);
        continue;
      } else {
        out.insert(key, serde_json::json!(format!("{:?}", v_node)));
      }
    }
  }
  Ok(out)
}

/// Convert a saphyr::Yaml node into serde_json::Value
fn yamlnode_to_json(n: &Yaml) -> Result<serde_json::Value, ParserError> {
  if n.is_null() {
    return Ok(serde_json::Value::Null);
  } else if n.is_boolean() {
    return Ok(serde_json::Value::Bool(n.as_bool().unwrap()));
  } else if n.is_integer() {
    return Ok(serde_json::Value::Number(serde_json::Number::from(
      n.as_integer().unwrap(),
    )));
  } else if n.is_floating_point() {
    let f = n.as_floating_point().unwrap();
    let num = serde_json::Number::from_f64(f)
      .ok_or_else(|| ParserError::YamlParseError("Invalid float".to_string()))?;
    return Ok(serde_json::Value::Number(num));
  } else if n.is_string() {
    return Ok(serde_json::Value::String(n.as_str().unwrap().to_string()));
  } else if n.is_sequence() {
    let mut v = Vec::new();
    if let Yaml::Sequence(seq) = n {
      for it in seq.iter() {
        v.push(yamlnode_to_json(it)?);
      }
    }
    return Ok(serde_json::Value::Array(v));
  } else if n.is_mapping() {
    let mut map = serde_json::Map::new();
    if let Yaml::Mapping(mvec) = n {
      for (k_node, v_node) in mvec.iter() {
        let key = k_node
          .as_str()
          .unwrap_or(&format!("{:?}", k_node))
          .to_string();
        map.insert(key, yamlnode_to_json(v_node)?);
      }
    }
    return Ok(serde_json::Value::Object(map));
  }
  Ok(serde_json::Value::Null)
}

/// Simple merging of JSON objects: values from `b` override `a` (deep merge for objects).
fn merge_json(a: &mut serde_json::Value, b: &serde_json::Value) {
  match (a, b) {
    (serde_json::Value::Object(map_a), serde_json::Value::Object(map_b)) => {
      for (k, v_b) in map_b.iter() {
        if let Some(v_a) = map_a.get_mut(k) {
          merge_json(v_a, v_b);
        } else {
          map_a.insert(k.clone(), v_b.clone());
        }
      }
    }
    (slot, v_b) => {
      *slot = v_b.clone();
    }
  }
}

/// Expand templates inside a serde_json::Value recursively.
fn expand_json_templates(
  v: &serde_json::Value,
  variables: &HashMap<String, serde_json::Value>,
  context: Option<&HashMap<String, serde_json::Value>>,
) -> Result<serde_json::Value, ParserError> {
  match v {
    serde_json::Value::String(s) => {
      let out = expand_string_templates(s, variables, context)?;
      Ok(serde_json::Value::String(out))
    }
    serde_json::Value::Array(arr) => {
      let mut out = Vec::new();
      for it in arr {
        out.push(expand_json_templates(it, variables, context)?);
      }
      Ok(serde_json::Value::Array(out))
    }
    serde_json::Value::Object(map) => {
      let mut out_map = serde_json::Map::new();
      for (k, vv) in map.iter() {
        out_map.insert(k.clone(), expand_json_templates(vv, variables, context)?);
      }
      Ok(serde_json::Value::Object(out_map))
    }
    other => Ok(other.clone()),
  }
}

/// Expand templates inside a single string.
/// Handles `{var}`, `{map[$key]}` and `{{ expr }}`.
fn expand_string_templates(
  s: &str,
  variables: &HashMap<String, serde_json::Value>,
  _context: Option<&HashMap<String, serde_json::Value>>,
) -> Result<String, ParserError> {
  // First handle {{ expr }} occurrences
  let mut result = String::new();
  let mut rest = s;
  while let Some(start) = rest.find("{{") {
    let (before, after_start) = rest.split_at(start);
    result.push_str(before);
    if let Some(end) = after_start.find("}}") {
      let expr = after_start[2..end].trim();
      let evaled = eval_expr(expr, variables)?;
      result.push_str(&evaled.to_string());
      rest = &after_start[end + 2..];
    } else {
      return Err(ParserError::YamlParseError(
        "Unclosed {{ expression".to_string(),
      ));
    }
  }
  result.push_str(rest);

  // Now handle { ... } templates (single braces)
  let mut out = String::new();
  let mut i = 0usize;
  let bytes = result.as_bytes();
  while i < bytes.len() {
    if bytes[i] == b'{' {
      if let Some(j) = result[i + 1..].find('}') {
        let inner = result[i + 1..i + 1 + j].trim();
        if let Some(br_idx) = inner.find('[') {
          // parse key and index
          let key = inner[..br_idx].trim();
          if inner.ends_with(']') {
            let idx = &inner[br_idx + 1..inner.len() - 1];
            let idx_clean = idx.trim().trim_matches('$').trim();
            let idx_val = if idx.trim().starts_with('$') {
              variables
                .get(idx_clean)
                .cloned()
                .unwrap_or(serde_json::Value::Null)
            } else {
              serde_json::Value::String(idx_clean.to_string())
            };
            if let Some(mapv) = variables.get(key) {
              if let serde_json::Value::Object(obj) = mapv {
                if let Some(serde_json::Value::Object(m)) = obj.get("map") {
                  let idxs = match idx_val {
                    serde_json::Value::String(ref s) => s.clone(),
                    other => other.to_string(),
                  };
                  if let Some(found) = m.get(&idxs) {
                    out.push_str(&found.to_string().trim_matches('"'));
                  } else {
                    return Err(ParserError::YamlParseError(format!(
                      "Missing key '{}' in map for variable '{}'",
                      idxs, key
                    )));
                  }
                } else if let Some(serde_json::Value::Object(m)) = obj.get("per_cluster") {
                  let idxs = match idx_val {
                    serde_json::Value::String(ref s) => s.clone(),
                    other => other.to_string(),
                  };
                  if let Some(found) = m.get(&idxs) {
                    out.push_str(&found.to_string().trim_matches('"'));
                  } else if let Some(def) = obj.get("default") {
                    out.push_str(&def.to_string().trim_matches('"'));
                  } else {
                    return Err(ParserError::YamlParseError(format!(
                      "Missing per_cluster key '{}' for '{}'",
                      idxs, key
                    )));
                  }
                } else if let Some(def) = obj.get("default") {
                  out.push_str(&def.to_string().trim_matches('"'));
                } else {
                  out.push_str(&mapv.to_string());
                }
              } else {
                out.push_str(&mapv.to_string().trim_matches('"'));
              }
            } else {
              return Err(ParserError::YamlParseError(format!(
                "Unknown variable '{}'",
                key
              )));
            }
            i = i + 1 + j + 1;
            continue;
          } else {
            return Err(ParserError::YamlParseError(
              "Malformed bracketed expression".to_string(),
            ));
          }
        } else {
          // simple {var}
          let varname = inner.trim_matches('$');
          if let Some(vv) = variables.get(varname) {
            match vv {
              serde_json::Value::String(sv) => out.push_str(sv),
              serde_json::Value::Number(n) => out.push_str(&n.to_string()),
              serde_json::Value::Array(arr) => {
                let s = arr
                  .iter()
                  .map(|x| x.to_string().trim_matches('"').to_string())
                  .collect::<Vec<_>>()
                  .join(",");
                out.push_str(&s);
              }
              serde_json::Value::Object(_) => {
                out.push_str(&vv.to_string());
              }
              serde_json::Value::Bool(b) => out.push_str(&b.to_string()),
              _ => out.push_str(&vv.to_string()),
            }
          } else {
            return Err(ParserError::YamlParseError(format!(
              "Unknown variable '{}'",
              varname
            )));
          }
          i = i + 1 + j + 1;
          continue;
        }
      } else {
        return Err(ParserError::YamlParseError(
          "Unclosed { in template".to_string(),
        ));
      }
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }

  Ok(out)
}

/// Evaluate a simple expression: supports integer literals, variables, and * operator.
fn eval_expr(
  expr: &str,
  variables: &HashMap<String, serde_json::Value>,
) -> Result<i64, ParserError> {
  let mut result: Option<i64> = None;
  for part in expr.split('*') {
    let ptrim = part.trim();
    let val = if let Ok(i) = ptrim.parse::<i64>() {
      i
    } else {
      let name = ptrim.trim_start_matches('$');
      if let Some(vv) = variables.get(name) {
        match vv {
          serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
              i
            } else {
              return Err(ParserError::EvalError(format!(
                "Non-integer value for variable '{}'",
                name
              )));
            }
          }
          serde_json::Value::String(s) => {
            if let Ok(i) = s.parse::<i64>() {
              i
            } else {
              return Err(ParserError::EvalError(format!(
                "Cannot parse '{}' as integer for variable '{}'",
                s, name
              )));
            }
          }
          _ => {
            return Err(ParserError::EvalError(format!(
              "Unsupported type for '{}' in expression",
              name
            )));
          }
        }
      } else {
        return Err(ParserError::EvalError(format!(
          "Unknown variable '{}' in expression",
          name
        )));
      }
    };
    if let Some(acc) = result {
      result = Some(acc * val);
    } else {
      result = Some(val);
    }
  }
  result.ok_or_else(|| ParserError::EvalError("Empty expression".to_string()))
}

/// Public parser entry. Returns intermediate ParsedCluster objects.
pub fn parse_clusters_configs_from_file(root: &Path) -> Result<Vec<ParsedCluster>, ParserError> {
  let yaml = Box::new(load_yaml_from_file(root)?);
  let base_dir = root
    .parent()
    .map(|p| p.to_owned())
    .unwrap_or_else(|| PathBuf::from("."));

  // Merge includes recursively
  debug!("Loading and merging included files...");
  let yaml = load_and_merge_includes(yaml, &base_dir)?;

  // FIXME
  panic!("Not tested from here on!!!");

  // Top-level variables collection
  let variables_map = collect_variables(&yaml, &base_dir)?;

  // default_params top-level
  let default_params_node =
    yaml_lookup(&yaml, "default_params").unwrap_or(&Yaml::Value(saphyr::Scalar::Null));
  let default_params = yamlnode_to_json(&default_params_node)?;

  // clusters
  let clusters_node = yaml_lookup(&yaml, "clusters")
    .ok_or_else(|| ParserError::YamlParseError("Missing top-level 'clusters' key".to_string()))?;

  if !clusters_node.is_mapping() {
    return Err(ParserError::YamlParseError(
      "'clusters' must be a mapping".to_string(),
    ));
  }

  let mut parsed_clusters = Vec::new();

  if let Yaml::Mapping(clusters_map) = &clusters_node {
    for (cluster_key_node, cluster_val_node) in clusters_map.iter() {
      let cluster_name = cluster_key_node.as_str().unwrap_or("").to_string();
      if cluster_name.is_empty() {
        return Err(ParserError::YamlParseError(
          "Cluster name must be a string".to_string(),
        ));
      }

      // Each cluster can have its own variables
      let mut combined_vars = variables_map.clone();

      if let Some(cluster_vars_node) = yaml_lookup(cluster_val_node, "variables") {
        let cluster_vars = parse_variables_block(&cluster_vars_node, &base_dir)?;
        combined_vars.extend(cluster_vars);
      }

      // Cluster-level default_params overriding top-level
      let mut cluster_default_params = default_params.clone();
      if let Some(cluster_dp) = yaml_lookup(cluster_val_node, "default_params") {
        let jp = yamlnode_to_json(&cluster_dp)?;
        merge_json(&mut cluster_default_params, &jp);
      }

      // scheduler
      let scheduler = yaml_lookup(cluster_val_node, "scheduler")
        .and_then(|n| Some(String::from_str(n.as_str()?)))
        .map(|s| s.unwrap().to_string());

      // configs
      let mut parsed_configs = Vec::new();

      if let Some(cfgs) = yaml_lookup(cluster_val_node, "configs") {
        if !cfgs.is_sequence() {
          return Err(ParserError::YamlParseError(
            "configs should be a sequence".to_string(),
          ));
        }
        if let Yaml::Sequence(cfg_seq) = &cfgs {
          for cfg_item in cfg_seq.iter() {
            // name (required)
            let name_templ = yaml_lookup(cfg_item, "name")
              .and_then(|n| Some(String::from_str(n.as_str().unwrap()).unwrap()))
              .ok_or_else(|| {
                ParserError::YamlParseError("Each config needs a 'name' string".to_string())
              })?;

            // params override for this config
            let mut cfg_params = cluster_default_params.clone();
            if let Some(p) = yaml_lookup(cfg_item, "params") {
              let jp = yamlnode_to_json(&p)?;
              merge_json(&mut cfg_params, &jp);
            }

            // extra_headers
            let mut extra_headers = Vec::new();
            if let Some(headers_node) = yaml_lookup(cfg_item, "extra_headers") {
              if let Yaml::Sequence(header_seq) = &headers_node {
                for h in header_seq.iter() {
                  if let Some(s) = h.as_str() {
                    extra_headers.push(s.to_string());
                  }
                }
              }
            }

            parsed_configs.push(ParsedConfig {
              name_template: name_templ,
              params: cfg_params,
              extra_headers,
            });
          }
        }
      }

      parsed_clusters.push(ParsedCluster {
        name: cluster_name,
        scheduler,
        default_params: cluster_default_params,
        configs: parsed_configs,
      });
    }
  }

  // Expand variables within configs and default_params
  let mut fully_expanded_clusters = Vec::new();

  for mut pc in parsed_clusters {
    // Expand default_params
    let expanded_dp = expand_json_templates(&pc.default_params, &variables_map, None)?;
    pc.default_params = expanded_dp;

    // Expand configs
    let mut expanded_configs = Vec::new();
    for cfg in pc.configs {
      // Expand name template
      let name_expanded = expand_string_templates(&cfg.name_template, &variables_map, None)?;

      // Expand params object recursively
      let params_expanded = expand_json_templates(&cfg.params, &variables_map, None)?;

      // Expand extra headers
      let mut ex_headers_expanded = Vec::new();
      for h in cfg.extra_headers {
        let s = expand_string_templates(&h, &variables_map, None)?;
        ex_headers_expanded.push(s);
      }

      expanded_configs.push(ParsedConfig {
        name_template: name_expanded,
        params: params_expanded,
        extra_headers: ex_headers_expanded,
      });
    }
    pc.configs = expanded_configs;

    fully_expanded_clusters.push(pc);
  }

  Ok(fully_expanded_clusters)
}
