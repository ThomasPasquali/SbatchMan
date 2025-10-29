use std::{
  collections::HashMap,
  ffi::{CStr, CString},
};

use pyo3::{PyResult, Python, types::PyDict};

use crate::core::{
  cluster_configs::ClusterConfig,
  parsers::variables::{BasicVar, CompleteVar, Scalar},
};

pub fn substitute_and_evaluate(
  template: &str,
  values: &HashMap<String, String>,
  var_map: &HashMap<String, &CompleteVar>,
  python_header: &Option<String>,
) -> String {
  // First, substitute simple variables
  let mut result = Substitutor::substitute_simple(template, values);

  // Then, substitute map references
  result = Substitutor::substitute_maps(&result, values, var_map);

  // Finally, evaluate Python expressions
  if result.contains("@py") {
    result = PythonEvaluator::evaluate(&result, python_header);
  }

  result
}

// Module for tracking variable dependencies
pub struct DependencyGraph {
  dependencies: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
  pub fn build(
    command: &str,
    preprocess: &Option<String>,
    postprocess: &Option<String>,
    var_map: &HashMap<String, &CompleteVar>,
  ) -> Self {
    let mut dependencies = HashMap::new();

    // Collect all strings to analyze
    let mut strings_to_analyze = vec![command];
    if let Some(p) = preprocess {
      strings_to_analyze.push(p);
    }
    if let Some(p) = postprocess {
      strings_to_analyze.push(p);
    }

    // Find direct dependencies in command strings
    for s in &strings_to_analyze {
      if let Some(vars) = get_variables_dependency(&s.to_string()) {
        for var in vars {
          dependencies.entry(var.to_string()).or_insert_with(Vec::new);
        }
      }
    }

    // Expand dependencies transitively
    Self::expand_transitive_dependencies(&mut dependencies, var_map);

    DependencyGraph { dependencies }
  }

  fn expand_transitive_dependencies(
    dependencies: &mut HashMap<String, Vec<String>>,
    var_map: &HashMap<String, &CompleteVar>,
  ) {
    let mut changed = true;
    while changed {
      changed = false;
      let keys: Vec<_> = dependencies.keys().cloned().collect();

      for key in keys {
        // Check if this variable's value contains other variables
        if let Some(var) = var_map.get(&key) {
          let transitive_deps = Self::find_dependencies_in_var(var);

          for dep in transitive_deps {
            if !dependencies.contains_key(&dep) {
              dependencies.insert(dep.clone(), Vec::new());
              changed = true;
            }

            let current_deps = dependencies.get(&key).unwrap().clone();
            if !current_deps.contains(&dep) {
              dependencies.get_mut(&key).unwrap().push(dep);
              changed = true;
            }
          }
        }
      }
    }
  }

  fn find_dependencies_in_var(var: &CompleteVar) -> Vec<String> {
    let mut deps = Vec::new();

    match var {
      CompleteVar::Scalar(s) => {
        if let Some(s_str) = scalar_to_string(s) {
          if let Some(vars) = get_variables_dependency(&s_str) {
            deps.extend(vars.into_iter().map(|s| s.to_string()));
          }
        }
      }
      CompleteVar::List(list) => {
        for scalar in list {
          if let Some(s_str) = scalar_to_string(scalar) {
            if let Some(vars) = get_variables_dependency(&s_str) {
              deps.extend(vars.into_iter().map(|s| s.to_string()));
            }
          }
        }
      }
      CompleteVar::StandardMap(map) => {
        for basic_var in map.values() {
          deps.extend(Self::find_dependencies_in_basic_var(basic_var));
        }
      }
      CompleteVar::ClusterMap(cm) => {
        if let Some(default) = &cm.default {
          deps.extend(Self::find_dependencies_in_basic_var(default));
        }
        for basic_var in cm.per_cluster.values() {
          deps.extend(Self::find_dependencies_in_basic_var(basic_var));
        }
      }
    }

    deps
  }

  fn find_dependencies_in_basic_var(var: &BasicVar) -> Vec<String> {
    let mut deps = Vec::new();

    match var {
      BasicVar::Scalar(s) => {
        if let Some(s_str) = scalar_to_string(s) {
          if let Some(vars) = get_variables_dependency(&s_str) {
            deps.extend(vars.into_iter().map(|s| s.to_string()));
          }
        }
      }
      BasicVar::List(list) => {
        for scalar in list {
          if let Some(s_str) = scalar_to_string(scalar) {
            if let Some(vars) = get_variables_dependency(&s_str) {
              deps.extend(vars.into_iter().map(|s| s.to_string()));
            }
          }
        }
      }
    }

    deps
  }

  fn has_dependencies(&self, var_name: &str) -> bool {
    self
      .dependencies
      .get(var_name)
      .map(|deps| !deps.is_empty())
      .unwrap_or(false)
  }

  fn get_dependencies(&self, var_name: &str) -> Vec<String> {
    self.dependencies.get(var_name).cloned().unwrap_or_default()
  }
}

// Module for resolving variables to their actual values
pub struct VariableResolver;

impl VariableResolver {
  pub fn resolve_for_cluster(
    cluster_config: &ClusterConfig,
    var_map: &HashMap<String, &CompleteVar>,
    dep_graph: &DependencyGraph,
  ) -> HashMap<String, Vec<String>> {
    let mut resolved = HashMap::new();

    for (name, var) in var_map {
      let values = Self::resolve_variable(cluster_config, var);
      resolved.insert(name.clone(), values);
    }

    resolved
  }

  fn resolve_variable(cluster_config: &ClusterConfig, var: &CompleteVar) -> Vec<String> {
    match var {
      CompleteVar::Scalar(s) => vec![scalar_to_string(s).unwrap_or_default()],
      CompleteVar::List(list) => list.iter().filter_map(|s| scalar_to_string(s)).collect(),
      CompleteVar::StandardMap(_) => {
        // Maps are not expanded directly, they're dereferenced during substitution
        vec![]
      }
      CompleteVar::ClusterMap(cm) => {
        let basic_var = cm
          .per_cluster
          .get(&cluster_config.cluster.cluster_name)
          .or(cm.default.as_ref());

        match basic_var {
          Some(BasicVar::Scalar(s)) => vec![scalar_to_string(s).unwrap_or_default()],
          Some(BasicVar::List(list)) => list.iter().filter_map(|s| scalar_to_string(s)).collect(),
          None => vec![],
        }
      }
    }
  }
}

// Module for generating Cartesian products
pub struct CartesianGenerator;

impl CartesianGenerator {
  pub fn generate(
    resolved_vars: &HashMap<String, Vec<String>>,
    dep_graph: &DependencyGraph,
  ) -> Vec<HashMap<String, String>> {
    // Separate independent and dependent variables
    let independent_vars: HashMap<_, _> = resolved_vars
      .iter()
      .filter(|(name, values)| !dep_graph.has_dependencies(name) && !values.is_empty())
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect();

    if independent_vars.is_empty() {
      return vec![HashMap::new()];
    }

    // Generate Cartesian product of independent variables
    Self::cartesian_product(&independent_vars)
  }

  fn cartesian_product(vars: &HashMap<String, Vec<String>>) -> Vec<HashMap<String, String>> {
    if vars.is_empty() {
      return vec![HashMap::new()];
    }

    let var_names: Vec<_> = vars.keys().cloned().collect();
    let var_values: Vec<_> = var_names.iter().map(|n| &vars[n]).collect();

    Self::cartesian_product_recursive(&var_names, &var_values, 0, &mut HashMap::new())
  }

  fn cartesian_product_recursive(
    names: &[String],
    values: &[&Vec<String>],
    index: usize,
    current: &mut HashMap<String, String>,
  ) -> Vec<HashMap<String, String>> {
    if index == names.len() {
      return vec![current.clone()];
    }

    let mut results = Vec::new();
    for value in values[index] {
      current.insert(names[index].clone(), value.clone());
      results.extend(Self::cartesian_product_recursive(
        names,
        values,
        index + 1,
        current,
      ));
      current.remove(&names[index]);
    }

    results
  }
}

// Module for string substitution
pub struct Substitutor;

impl Substitutor {
  fn substitute_simple(template: &str, values: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (name, value) in values {
      let pattern = format!("${{{}}}", name);
      result = result.replace(&pattern, value);
    }

    result
  }

  fn substitute_maps(
    template: &str,
    values: &HashMap<String, String>,
    var_map: &HashMap<String, &CompleteVar>,
  ) -> String {
    let mut result = template.to_string();

    // Pattern: ${MAP_VAR}[${KEY_VAR}] or ${MAP_VAR}[literal_key]
    let re = regex::Regex::new(r"\$\{([^}]+)\}\[([^\]]+)\]").unwrap();

    loop {
      let mut changed = false;

      result = re
        .replace_all(&result, |caps: &regex::Captures| {
          let map_name = &caps[1];
          let key_expr = &caps[2];

          // Resolve the key expression
          let key = if key_expr.starts_with("${") && key_expr.ends_with("}") {
            let key_var = &key_expr[2..key_expr.len() - 1];
            values.get(key_var).map(|s| s.as_str()).unwrap_or("")
          } else {
            key_expr
          };

          // Look up in the map
          if let Some(CompleteVar::StandardMap(map)) = var_map.get(map_name) {
            if let Some(basic_var) = map.get(key) {
              changed = true;
              return match basic_var {
                BasicVar::Scalar(s) => scalar_to_string(s).unwrap_or_default(),
                BasicVar::List(_) => {
                  // Lists in maps need special handling
                  format!("${{{}}}[{}]", map_name, key)
                }
              };
            }
          }

          caps[0].to_string()
        })
        .to_string();

      if !changed {
        break;
      }
    }

    result
  }
}

// Module for Python evaluation
pub struct PythonEvaluator;

impl PythonEvaluator {
  fn evaluate(template: &str, python_header: &Option<String>) -> String {
    Python::attach(|py| {
      let mut result = template.to_string();
      let re = regex::Regex::new(r"@py\s+((?s).*?)(?:@py|$)").unwrap();

      for caps in re.captures_iter(template) {
        let expr = caps[1].trim();

        match Self::eval_python(py, expr, python_header) {
          Ok(value) => {
            result = result.replace(&caps[0], &value);
          }
          Err(e) => {
            eprintln!("Python evaluation error: {}", e);
          }
        }
      }

      result
    })
  }

  fn eval_python(py: Python, expr: &str, header: &Option<String>) -> PyResult<String> {
    let locals = PyDict::new(py);

    // Execute header if provided
    if let Some(header_code) = header {
      py.run(
        &CString::new(header_code.as_str()).unwrap().as_c_str(),
        None,
        Some(&locals),
      )?;
    }

    // Evaluate the expression
    let result = py.eval(&CString::new(expr).unwrap().as_c_str(), None, Some(&locals))?;
    Ok(result.to_string())
  }
}

// Helper function
pub fn scalar_to_string(scalar: &Scalar) -> Option<String> {
  match scalar {
    Scalar::String(s) => Some(s.clone()),
    Scalar::Int(i) => Some(i.to_string()),
    Scalar::Float(f) => Some(f.to_string()),
    Scalar::Bool(b) => Some(b.to_string()),
    Scalar::File(f) => Some(f.clone()),
    Scalar::Directory(d) => Some(d.clone()),
    Scalar::Python(code) => Some(code.clone()),
  }
}

pub fn get_variables_dependency(value: &String) -> Option<Vec<&str>> {
  let mut variables = Vec::new();
  let bytes = value.as_bytes();
  let mut i = 0;

  while i < bytes.len() {
    if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
      i += 2;
      let start = i;

      while i < bytes.len() && bytes[i] != b'}' {
        i += 1;
      }

      if i < bytes.len() && bytes[i] == b'}' {
        let var_name = &value[start..i];
        if !var_name.is_empty() {
          variables.push(var_name);
        }
        i += 1;
      }
    } else {
      i += 1;
    }
  }

  if variables.is_empty() {
    None
  } else {
    Some(variables)
  }
}
