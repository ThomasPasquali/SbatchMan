use crate::core::parsers::{
  ParserError,
  multi_hashmap::MultiHashMap,
  template_parser::{Template, TemplateSegment, Variable, VariableSegment},
  variables::{BasicVar, CompleteVar, ListVar, MapKind, MapVar, PythonVar, Scalar},
};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

struct ListNode<'a> {
  list_var: &'a ListVar,
  index: usize,
}

impl ListNode<'_> {
  fn get_value(&self) -> Result<Scalar, ParserError> {
    self.list_var.get(self.index)
  }
}

enum MapKey {
  Constant(String),
  Variable(String),
}

struct MapNode<'a> {
  map_var: &'a MapVar,
  key: MapKey,
  index: usize,
}

impl MapNode<'_> {
  fn get_value(&self, key: &str) -> Result<Scalar, ParserError> {
    self.map_var.get(key).and_then(|value| match value {
      BasicVar::List(list) => list.get(self.index),
      BasicVar::Scalar(scalar) => Ok(scalar.clone()),
    })
  }
}

struct PythonNode<'a> {
  python_var: &'a PythonVar,
  computed_cache: Scalar,
}

impl PythonNode<'_> {
  fn evaluate(&mut self, context: &CombinationGenerator) -> Result<(), ParserError> {
    // TODO
    Ok(())
  }

  fn get_value(&self) -> Result<Scalar, ParserError> {
    Ok(self.computed_cache.clone())
  }
}

enum DynamicVarNode<'a> {
  List(ListNode<'a>),     // Represents a list variable (stores the index)
  Map(MapNode<'a>),       // Represents a Map (stores the key variable name and index)
  Python(PythonNode<'a>), // Represents Python Code (stores the code/expression)
}

pub(crate) struct CombinationGenerator<'a> {
  variable_map: &'a MultiHashMap<String, CompleteVar>,
  dynamic_vars: HashMap<String, NodeIndex>,
  dependency_graph: DiGraph<DynamicVarNode<'a>, ()>,
  cluster: String,
}

impl<'a> CombinationGenerator<'a> {
  fn new(variable_map: &'a MultiHashMap<String, CompleteVar>, cluster: &str) -> Self {
    Self {
      variable_map,
      cluster: cluster.to_string(),
      dynamic_vars: HashMap::new(),
      dependency_graph: DiGraph::new(),
    }
  }

  /// Helper: Find a variable by name, or return error.
  fn get_var(
    variable_map: &'a MultiHashMap<String, CompleteVar>,
    name: &str,
  ) -> Result<&'a CompleteVar, ParserError> {
    variable_map
      .get(name)
      .ok_or_else(|| ParserError::EvalError(format!("Variable '{}' not found", name)))
  }

  fn get_map_var(
    variable_map: &'a MultiHashMap<String, CompleteVar>,
    name: &str,
  ) -> Result<&'a MapVar, ParserError> {
    match Self::get_var(variable_map, name)? {
      CompleteVar::Map(map_var) => Ok(map_var),
      _ => Err(ParserError::EvalError(format!(
        "Variable '{}' is not a map variable",
        name
      ))),
    }
  }

  pub fn register_segments(
    &mut self,
    template: &Template,
  ) -> Result<(), ParserError> {
    for segment in template.segments.iter() {
      self.register_segment(segment)?;
    }
    Ok(())
  }

  /// Register a variable segment into the combination generator.
  /// If the segment introduces dynamic behavior (lists, maps, python), add nodes to the dependency graph.
  fn register_segment(&mut self, segment: &TemplateSegment) -> Result<Option<NodeIndex>, ParserError> {
    // Skip literals
    let segment = match segment {
      TemplateSegment::VariableSegment(vs) => vs,
      _ => return Ok(None),
    };

    let segment_name = segment.name();

    if self.dynamic_vars.contains_key(&segment_name) {
      return Ok(None); // Already registered
    }

    match segment {
      VariableSegment::Variable(_) => {
        // 1. Resolve the variable
        let var = Self::get_var(&self.variable_map, &segment_name)?;

        // 2. Handle types
        match var {
          CompleteVar::BasicVar(BasicVar::List(list_var)) => {
            Ok(Some(self.add_list_variable(&segment_name, list_var)))
          }
          CompleteVar::BasicVar(BasicVar::Scalar(_)) => {
            // Scalars are constant; no graph node needed.
            Ok(None)
          }
          CompleteVar::Python(python_var) => {
            Ok(Some(self.add_python_variable(&segment_name, python_var)?))
          }
          CompleteVar::Map(map_var) => {
            // Special logic: Usage without an explicit key
            match map_var.kind() {
              MapKind::Cluster => {
                // Implicit key: The current cluster name
                Ok(Some(self.add_map_variable_with_const_key(&segment_name, map_var, &self.cluster.clone())))
              }
              MapKind::Standard => {
                return Err(ParserError::EvalError(format!(
                  "Map variable '{}' requires a key for lookup",
                  segment_name
                )));
              }
            }
          }
        }
      }

      VariableSegment::ConstantMap { map_name, key } => {
        let map_var = Self::get_map_var(&self.variable_map, &map_name)?;
        Ok(Some(self.add_map_variable_with_const_key(&segment_name, map_var, &key)))
      }

      VariableSegment::VariableMap { map_name, var_name } => {
        let map_var = Self::get_map_var(&self.variable_map, &map_name)?;
        Ok(self.add_map_variable_with_var_key(&segment_name, map_var, &var_name))
      }
    }
  }

  fn add_list_variable(&mut self, var_name: &str, var: &'a ListVar) -> NodeIndex {
    let node = DynamicVarNode::List(ListNode {
      list_var: var,
      index: 0,
    });
    let idx = self.dependency_graph.add_node(node);
    self.dynamic_vars.insert(var_name.to_string(), idx);
    idx
  }

  fn add_map_variable_with_var_key(&mut self, var_name: &str, var: &'a MapVar, key_var_name: &str) -> Result<NodeIndex, ParserError> {
    let node = DynamicVarNode::Map(MapNode {
      map_var: var,
      key: MapKey::Variable(key_var_name.to_string()),
      index: 0,
    });
    let idx = self.dependency_graph.add_node(node);

    // Register dependency on the key variable
    let dep_idx = self.register_segment(&TemplateSegment::VariableSegment(VariableSegment::Variable(key_var_name.to_string())))?;
    if let Some(dep_idx) = dep_idx {
      self.dependency_graph.add_edge(dep_idx, idx, ());
    }
    self.dynamic_vars.insert(var_name.to_string(), idx);
    Ok(idx)
  }

  fn add_map_variable_with_const_key(&mut self, var_name: &str, var: &'a MapVar, key: &str) -> NodeIndex {
    let node = DynamicVarNode::Map(MapNode {
      map_var: var,
      key: MapKey::Constant(key.to_string()),
      index: 0,
    });
    let idx = self.dependency_graph.add_node(node);
    self.dynamic_vars.insert(var_name.to_string(), idx);
    idx
  }

  fn add_python_variable(&mut self, var_name: &str, var: &'a PythonVar) -> Result<NodeIndex, ParserError> {
    let node = DynamicVarNode::Python(PythonNode {
      python_var: var,
      computed_cache: Scalar::String(String::new()),
    });
    let node_idx = self.dependency_graph.add_node(node);

    // Register dependencies. For each variable segment in the Python template, add an edge.
    // The edges are from dependency -> dependent (i.e., from the variable used in Python to the Python node)
    for segment in var.template.segments.iter() {
      let dep_idx = self.register_segment(segment)?;
      if let Some(dep_idx) = dep_idx {
        self.dependency_graph.add_edge(dep_idx, node_idx, ());
      }
    }

    self.dynamic_vars.insert(var_name.to_string(), node_idx);
    Ok(node_idx)
  }

  pub(crate) fn try_iter(&'a self) -> Result<CombinationIterator<'a>, ParserError> {
    let sorted_nodes = petgraph::algo::toposort(&self.dependency_graph, None)
      .map_err(|_| ParserError::CyclicVariableDependency())?;
    let size = sorted_nodes.len();

    Ok(CombinationIterator {
      generator: self,
      sorted_nodes,
      finished: false,
    })
  }
}

// The Iterator now owns the state (indices)
pub(crate) struct CombinationIterator<'a> {
  generator: &'a CombinationGenerator<'a>,
  sorted_nodes: Vec<NodeIndex>, // Topological order (dependency -> dependent)
  finished: bool,
}

impl<'a> CombinationIterator<'a> {
  fn get_value(&self, var_name: &str) -> Result<Scalar, ParserError> {
    if let Some(&node_idx) = self.generator.dynamic_vars.get(var_name) {
      let node = &self.generator.dependency_graph[node_idx];
      match node {
        DynamicVarNode::List(list_node) => list_node.get_value(),
        DynamicVarNode::Map(map_node) => {
          // Resolve the key variable first
          self
            .get_value(&map_node.key_var_name)
            .and_then(|key_scalar| {
              let key_str = key_scalar.to_string();
              map_node.get_value(&key_str)
            })
        }
        DynamicVarNode::Python(python_node) => python_node.get_value(),
      }
    } else {
      if let Some(var) = self.generator.variable_map.get(var_name) {
        match var {
          CompleteVar::BasicVar(BasicVar::Scalar(scalar)) => Ok(scalar.clone()),
          _ => Err(ParserError::EvalError(format!(
            "Variable '{}' is not a scalar",
            var_name
          ))),
        }
      } else {
        Err(ParserError::EvalError(format!(
          "Variable '{}' not found",
          var_name
        )))
      }
    }
  }
}

impl<'a> Iterator for CombinationIterator<'a> {
  // Return the Context Snapshot (Map of all resolved variables)
  type Item = &'a CombinationGenerator<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.finished {
      return None;
    }

    // Binary counter algorithm

    // Iterate backwards through sorted nodes
    for &node_idx in self.sorted_nodes.iter().rev() {
      // Calculate size dynamically (because size might change based on previous vars)
      let size = 1; // self.get_node_size(node_idx, ...);
      let current_idx = self.indices.get_mut(&node_idx).unwrap();

      if *current_idx < size - 1 {
        *current_idx += 1;
        advanced = true;
        break; // Successfully incremented, stop cascading
      } else {
        *current_idx = 0; // Reset and carry over to next node
      }
    }

    if !advanced {
      self.finished = true;
    }

    Some(context)
  }
}
