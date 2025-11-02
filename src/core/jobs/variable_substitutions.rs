use std::{
  collections::{HashMap, HashSet},
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
  dep_graph: &DependencyGraph,
  python_header: &Option<String>,
) -> String {
  // First, add all dependent variables to the values map
  let mut all_values = values.clone();

  // Add dependent variables by resolving them from var_map
  for (var_name, var) in var_map {
    if !all_values.contains_key(var_name) && dep_graph.has_dependencies(var_name) {
      // This is a dependent variable, get its initial value
      if let Some(initial_value) = get_initial_value(var) {
        all_values.insert(var_name.clone(), initial_value);
      }
    }
  }

  // Recursively resolve all dependencies
  let resolved_values = resolve_dependencies(&all_values, dep_graph);

  // First, substitute simple variables
  let mut result = Substitutor::substitute_simple(template, &resolved_values);

  // Then, substitute map references
  result = Substitutor::substitute_maps(&result, &resolved_values, var_map);

  // Finally, evaluate Python expressions
  if result.contains("!py") {
    result = PythonEvaluator::evaluate(&result, python_header);
  }

  result
}

fn get_initial_value(var: &CompleteVar) -> Option<String> {
  match var {
    CompleteVar::Scalar(s) => scalar_to_string(s),
    CompleteVar::List(list) => {
      // For lists, we shouldn't be here in the cartesian product phase
      // But if we are, just take the first value
      list.first().and_then(|s| scalar_to_string(s))
    }
    CompleteVar::StandardMap(_) | CompleteVar::ClusterMap(_) => {
      // Maps don't have direct values
      None
    }
  }
}

fn resolve_dependencies(
  values: &HashMap<String, String>,
  dep_graph: &DependencyGraph,
) -> HashMap<String, String> {
  let mut resolved = values.clone();
  let mut changed = true;
  let mut iterations = 0;
  const MAX_ITERATIONS: usize = 100; // Prevent infinite loops

  while changed && iterations < MAX_ITERATIONS {
    changed = false;
    iterations += 1;

    // Get a sorted list of variables to process
    // (we need deterministic ordering for consistent results)
    let mut var_names: Vec<_> = resolved.keys().cloned().collect();
    var_names.sort();

    for var_name in var_names {
      // Check if this variable has dependencies
      if dep_graph.has_dependencies(&var_name) {
        let current_value = resolved.get(&var_name).unwrap().clone();

        // Try to substitute dependencies in the current value
        let new_value = Substitutor::substitute_simple(&current_value, &resolved);

        // If the value changed, mark that we need another iteration
        if new_value != current_value {
          resolved.insert(var_name, new_value);
          changed = true;
        }
      }
    }
  }

  if iterations >= MAX_ITERATIONS {
    eprintln!(
      "Warning: Maximum dependency resolution iterations reached. Possible circular dependency."
    );
  }

  resolved
}

// Module for tracking variable dependencies
#[derive(Debug)]
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
    match var {
      CompleteVar::Scalar(scalar) => {
        // Convert scalar to single-element vector
        if let Some(s) = scalar_to_string(scalar) {
          resolved.insert(name.clone(), vec![s]);
        }
      }
      CompleteVar::List(list) => {
        // Convert list items to strings
        let values: Vec<String> = list
          .iter()
          .filter_map(|item| scalar_to_string(item))
          .collect();
        if !values.is_empty() {
          resolved.insert(name.clone(), values);
        }
      }
      CompleteVar::StandardMap(_) => {
        // Maps are not included - they're used for lookups, not expansion
      }
      CompleteVar::ClusterMap(cluster_map) => {
        // Extract values for the current cluster
        if let Some(basic_var) = cluster_map.get(&cluster_config.cluster.cluster_name) {
          match basic_var {
            BasicVar::Scalar(scalar) => {
              if let Some(s) = scalar_to_string(scalar) {
                resolved.insert(name.clone(), vec![s]);
              }
            }
            BasicVar::List(list) => {
              let values: Vec<String> = list
                .iter()
                .filter_map(|item| scalar_to_string(item))
                .collect();
              if !values.is_empty() {
                resolved.insert(name.clone(), values);
              }
            }
          }
        }
      }
    }
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
    command: &String,
    preprocess: &Option<String>,
    postprocess: &Option<String>,
  ) -> Vec<HashMap<String, String>> {
    // Get all used variables
    let used_vars = get_all_variable_dependencies(dep_graph, command, &preprocess, &postprocess);

    // Only consider independent variables that are actually used
    let independent_vars: HashMap<_, _> = resolved_vars
      .iter()
      .filter(|(name, _)| used_vars.contains(*name) && !dep_graph.has_dependencies(name))
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
  pub fn substitute(
    template: &str,
    values: &HashMap<String, String>,
    var_map: &HashMap<String, &CompleteVar>,
  ) -> String {
    // First substitute maps (which may reference variables)
    let after_maps = Self::substitute_maps(template, values, var_map);
    
    // Then substitute simple variables
    Self::substitute_simple(&after_maps, values)
  }

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

    // Keep substituting until no more changes (handles nested substitutions)
    loop {
      let mut changed = false;

      result = re
        .replace_all(&result, |caps: &regex::Captures| {
          let map_name = &caps[1];
          let key_expr = &caps[2];

          // Resolve the key expression
          let key = if key_expr.starts_with("${") && key_expr.ends_with("}") {
            // Variable key: ${KEY_VAR}
            let key_var = &key_expr[2..key_expr.len() - 1];
            values.get(key_var).map(|s| s.as_str()).unwrap_or("")
          } else {
            // Literal key
            key_expr
          };

          // Look up in the map
          if let Some(CompleteVar::StandardMap(map)) = var_map.get(map_name) {
            if let Some(basic_var) = map.get(key) {
              changed = true;
              return match basic_var {
                BasicVar::Scalar(s) => scalar_to_string(s).unwrap_or_default(),
                BasicVar::List(_) => {
                  // Lists in maps would need expansion - for now keep the pattern
                  format!("${{{}}}[{}]", map_name, key)
                }
              };
            }
          }

          // If no substitution happened, return original
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
      let re = regex::Regex::new(r"!py\s+((?s).*?)(?:!py|$)").unwrap();

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

pub fn get_all_variable_dependencies(
  dep_graph: &DependencyGraph,
  command: &String,
  preprocess: &Option<String>,
  postprocess: &Option<String>,
) -> HashSet<String> {
  let mut used = HashSet::new();

  // Collect directly referenced variables
  for expr in [&Some(command.to_owned()), preprocess, postprocess].iter() {
    if let Some(comm) = expr {
      if let Some(vars) = get_variables_dependency(comm) {
        for v in vars {
          used.insert(v.to_string());
        }
      }
    }
  }

  // Expand transitively
  let mut queue: Vec<_> = used.clone().into_iter().collect();
  while let Some(var) = queue.pop() {
    for dep in dep_graph.get_dependencies(&var) {
      if used.insert(dep.clone()) {
        queue.push(dep.clone());
      }
    }
  }

  used
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

pub fn recursive_substitute(value: &str, vars: &HashMap<String, String>) -> String {
  let mut result = value.to_string();
  loop {
    let mut changed = false;
    let mut new_value = result.clone();
    for (name, val) in vars {
      let pattern = format!("${{{}}}", name);
      if new_value.contains(&pattern) {
        new_value = new_value.replace(&pattern, val);
        changed = true;
      }
    }
    if !changed {
      break;
    }
    result = new_value;
  }
  result
}
