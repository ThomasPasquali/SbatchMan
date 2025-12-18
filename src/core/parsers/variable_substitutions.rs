use std::collections::HashMap;

use lazy_regex::{Lazy, Regex, lazy_regex};
use petgraph::{Graph, algo::toposort, graph::NodeIndex};

use crate::core::parsers::{
  ParserError, multi_hashmap::MultiHashMap, variables::{BasicVar, CompleteVar, Scalar, Variable}
};

// Matches ${var_name}
pub static VARIABLE_REX: Lazy<Regex> = lazy_regex!(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}");
// Matches $var_name[${index}]
pub static MAP_REX: Lazy<Regex> =
  lazy_regex!(r"\$([a-zA-Z_][a-zA-Z0-9_]*)\[\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}\]");

struct Node {
  node_index: NodeIndex,
  values: Vec<String>,
}

/// Substitute variables in the input string using the provided variable definitions.
/// Returns a vector of all possible combinations after substitutions.
pub fn resolve_variable_substitutions(
  mut input_string: String,
  variables: MultiHashMap<String, Variable>,
) -> Result<Vec<String>, ParserError> {
  let mut dag = Graph::new();
  let mut parsed_vars: HashMap<String, Node>;
  // Create the variable dependency DAG by inspecting the dependencies of each variable found in the input string
  create_variables_dag(&mut input_string, &variables, &mut dag, &mut parsed_vars)?;
  // Topologically sort the DAG to get the order of variable substitutions
  let sorted_nodes = toposort(&dag, None).map_err(|_| ParserError::CyclicDependency())?;

  // For each variable in topological order, perform substitutions
  for node in sorted_nodes {
    let var_name = dag[node];
    let var_data = parsed_vars
      .get(&var_name)
      .ok_or(ParserError::UndefinedVariable(var_name.to_string()))?;
    let variable = variables
      .get(&var_name)
      .ok_or(ParserError::UndefinedVariable(var_name.to_string()))?;

    // Get the possible values for the variable
    evaluate_variable(variable, parsed_vars);
  }

  Ok(vec![])
}

/// Insert variables in the DAG. For each variable found in the input string, add it to the DAG along with its dependencies.
/// - For each variable found, if not already in the DAG, add it as a node
/// - For each dependency of the variable, add an edge from the dependency to the variable
/// - If a dependency is not found in the variables map, return an error
fn create_variables_dag(
  input_string: &mut str,
  variables: &MultiHashMap<String, Variable>,
  dag: &mut Graph<String, ()>,
  parsed_vars: &mut HashMap<String, Node>,
) -> Result<(), ParserError> {
  // Find all variables using regex
  let re = &*VARIABLE_REX;
  for cap in re.captures_iter(input_string) {
    let var_name = &cap[1];
    
    // Check if the variable is defined
    let variable = variables
      .get(var_name)
      .ok_or(ParserError::UndefinedVariable(var_name.to_string()))?;
    
    // If the variable is not already in the DAG, add it
    if !parsed_vars.contains_key(var_name) {
      // Add the variable to the DAG and to the parsed_vars map
      let dag_node = dag.add_node(var_name.to_string());
      parsed_vars.insert(var_name.to_string(), Node {
        node_index: dag_node,
        values: vec![],
      });

      // Find dependencies of the variable and add edges in the DAG
      build_variable_dependency_graph(variable, dag_node, &parsed_vars, variables, dag);
    }
  }

  // Find occurrences of $variable[${index}] using regex and add the index as dependency for the variable
  let re_indexed = &*MAP_REX;
  for cap in re_indexed.captures_iter(input_string) {
    let map_name = &cap[1]; // variable
    let index_name = &cap[2]; // index
    // Add the index variable as dependency for the variable
    let map_node = parsed_vars
      .get(map_name)
      .ok_or(ParserError::UndefinedVariable(map_name.to_string()))?;
    let map_var = variables
      .get(map_name)
      .ok_or(ParserError::UndefinedVariable(map_name.to_string()))?;
    let index_node = parsed_vars
      .get(index_name)
      .ok_or(ParserError::UndefinedVariable(index_name.to_string()))?;

    // Ensure that the map variable is indeed a StandardMap
    if !map_var.contents.is_standard_map() {
      return Err(ParserError::InvalidIndexedVariable(map_name.to_string()));
    }

    dag.add_edge(index_node.node_index, map_node.node_index, ());
  }
  Ok(())
}

/// Build the variable dependency graph by adding edges from dependencies to the variable node
/// - For each dependency of the variable, add an edge from the dependency to the variable node
/// - If a dependency is not found in the variables map, return an error
fn build_variable_dependency_graph(
  variable: &Variable,
  dag_node: NodeIndex,
  parsed_vars: &HashMap<String, Node>,
  variables: &HashMap<String, Variable>,
  dag: &mut Graph<String, ()>,
) -> Result<(), ParserError> {
  // If the variable is defined in variables, get its dependencies. Otherwise, return an error
  for dependency in get_dependencies(variable) {
    // Check if the dependency is already in the DAG
    if let Some(dep_node) = parsed_vars.get(&dependency) {
      dag.add_edge(dep_node.node_index, dag_node, ());
    } else if variables.contains_key(&dependency) {
      // Dependency variable is defined but not yet in the DAG, add it
      let dep_node = dag.add_node(dependency);
      dag.add_edge(dep_node, dag_node, ());
    } else {
      // Dependency variable is not defined, return an error
      return Err(ParserError::UndefinedVariable(dependency));
    }
  }
  Ok(())
}

/// Extract variables referenced in a Variable
fn get_dependencies(variable: &Variable) -> Vec<String> {
  let mut deps: Vec<String> = Vec::new();
  match &variable.contents {
    CompleteVar::BasicVar(v) => {
      deps.extend(get_basic_var_dependencies(v));
    },
    CompleteVar::StandardMap(map) => {
      for (_, basic_var) in map.iter() {
        deps.extend(get_basic_var_dependencies(basic_var));
      }
    }
    CompleteVar::ClusterMap(cluster_map) => {
      if let Some(default) = &cluster_map.default {
        deps.extend(get_basic_var_dependencies(default));
      }
      for (_, basic_var) in cluster_map.per_cluster.iter() {
        deps.extend(get_basic_var_dependencies(basic_var));
      }
    }
  }
  deps
}

/// Extract variables referenced in a BasicVar
fn get_basic_var_dependencies(basic_var: &BasicVar) -> Vec<String> {
  let mut deps: Vec<String> = Vec::new();
  match basic_var {
    BasicVar::Scalar(s) => {
      deps.extend(get_scalar_dependencies(s));
    }
    BasicVar::List(l) => {
      for item in l.iter() {
        deps.extend(get_scalar_dependencies(item));
      }
    }
    BasicVar::Python(s) => {
      deps.extend(get_variables_from_string(s));
    }
  }
  deps
}

/// Extract variables referenced in a Scalar
fn get_scalar_dependencies(s: &Scalar) -> Vec<String> {
  let mut deps: Vec<String> = Vec::new();
  if let Scalar::String(s) = s {
    deps.extend(get_variables_from_string(s));
  }
  deps
}

/// Extract variable names referenced in string using the ${var_name} syntax
fn get_variables_from_string(s: &str) -> Vec<String> {
  let mut vars: Vec<String> = Vec::new();
  let re = &*VARIABLE_REX;
  for cap in re.captures_iter(s) {
    let var_name = &cap[1];
    vars.push(var_name.to_string());
  }
  vars
}

/// Evaluate a variable. Check the values of its dependencies and compute its own values by performing a cartesian product of all combinations.
/// All its dependencies must have already been evaluated.
fn evaluate_variable(
  variable: &Variable,
  parsed_vars: &mut HashMap<String, Node>,
) -> Result<Vec<String>, ParserError> {
  let var_name = &variable.name;
  
  // Get all possible base values for this variable
  let base_values = match &variable.contents {
    CompleteVar::BasicVar(basic_var) => get_basic_var_values(basic_var)?,
    CompleteVar::StandardMap(map) => {
      // For maps, we need to handle them specially as they're accessed via $var[${index}]
      // For now, return empty as maps are resolved during substitution phase
      vec![]
    }
    CompleteVar::ClusterMap(cluster_map) => {
      // Similar to StandardMap, these are context-dependent
      vec![]
    }
  };
  
  // For each base value, perform variable substitutions using dependency values
  let mut result_values = Vec::new();
  
  for base_value in base_values {
    // Get all variables referenced in this base value
    let referenced_vars = get_variables_from_string(&base_value);
    
    if referenced_vars.is_empty() {
      // No substitutions needed
      result_values.push(base_value);
    } else {
      // Collect all possible values for each referenced variable
      let mut dep_value_sets: Vec<(String, Vec<String>)> = Vec::new();
      for dep_name in &referenced_vars {
        let dep_node = parsed_vars
          .get(dep_name)
          .ok_or(ParserError::UndefinedVariable(dep_name.to_string()))?;
        dep_value_sets.push((dep_name.clone(), dep_node.values.clone()));
      }
      
      // Create cartesian product iterator
      let mut cart_iter = CartesianProductIterator::new(dep_value_sets);
      
      // For each combination, substitute variables in the base value
      while let Some(substitution_map) = cart_iter.next() {
        let substituted = substitute_variables(&base_value, &substitution_map);
        result_values.push(substituted);
      }
    }
  }
  
  // Update the node with computed values
  if let Some(node) = parsed_vars.get_mut(var_name) {
    node.values = result_values.clone();
  }
  
  Ok(result_values)
}

/// Iterator that lazily generates cartesian products of variable value sets
struct CartesianProductIterator {
  // List of (variable_name, possible_values) pairs
  value_sets: Vec<(String, Vec<String>)>,
  // Current indices for each variable
  indices: Vec<usize>,
  // Whether we've exhausted all combinations
  exhausted: bool,
}

impl CartesianProductIterator {
  fn new(value_sets: Vec<(String, Vec<String>)>) -> Self {
    let exhausted = value_sets.is_empty() || value_sets.iter().any(|(_, vals)| vals.is_empty());
    let indices = vec![0; value_sets.len()];
    
    CartesianProductIterator {
      value_sets,
      indices,
      exhausted,
    }
  }
  
  fn next(&mut self) -> Option<HashMap<String, String>> {
    if self.exhausted {
      return None;
    }
    
    // Build current combination
    let mut result = HashMap::new();
    for (i, (var_name, values)) in self.value_sets.iter().enumerate() {
      result.insert(var_name.clone(), values[self.indices[i]].clone());
    }
    
    // Advance to next combination (like incrementing a mixed-radix number)
    let mut carry = true;
    for i in (0..self.indices.len()).rev() {
      if carry {
        self.indices[i] += 1;
        if self.indices[i] < self.value_sets[i].1.len() {
          carry = false;
        } else {
          self.indices[i] = 0;
        }
      }
    }
    
    // If we still have carry, we've exhausted all combinations
    if carry {
      self.exhausted = true;
    }
    
    Some(result)
  }
}

/// Substitute variables in a string using the provided substitution map
/// This is O(n) where n is the length of the string
fn substitute_variables(template: &str, substitutions: &HashMap<String, String>) -> String {
  let mut result = String::with_capacity(template.len());
  let mut chars = template.chars().peekable();
  
  while let Some(ch) = chars.next() {
    if ch == '$' && chars.peek() == Some(&'{') {
      chars.next(); // consume '{'
      
      // Extract variable name
      let mut var_name = String::new();
      let mut found_closing = false;
      
      while let Some(&next_ch) = chars.peek() {
        if next_ch == '}' {
          chars.next(); // consume '}'
          found_closing = true;
          break;
        }
        var_name.push(next_ch);
        chars.next();
      }
      
      if found_closing {
        // Substitute the variable
        if let Some(value) = substitutions.get(&var_name) {
          result.push_str(value);
        } else {
          // Variable not in substitution map, keep original
          result.push_str("${");
          result.push_str(&var_name);
          result.push('}');
        }
      } else {
        // Malformed variable reference, keep as-is
        result.push('$');
        result.push('{');
        result.push_str(&var_name);
      }
    } else {
      result.push(ch);
    }
  }
  
  result
}

/// Get the base string values from a BasicVar
fn get_basic_var_values(basic_var: &BasicVar) -> Result<Vec<String>, ParserError> {
  match basic_var {
    BasicVar::Scalar(s) => Ok(vec![scalar_to_string(s)]),
    BasicVar::List(l) => Ok(l.iter().map(scalar_to_string).collect()),
    BasicVar::Python(s) => Ok(vec![s.clone()]),
  }
}

/// Convert a Scalar to its string representation
fn scalar_to_string(scalar: &Scalar) -> String {
  match scalar {
    Scalar::String(s) => s.clone(),
    Scalar::Int(i) => i.to_string(),
    Scalar::Float(f) => f.to_string(),
    Scalar::Bool(b) => b.to_string(),
    Scalar::File(f) => f.clone(),
    Scalar::Directory(d) => d.clone(),
  }
}