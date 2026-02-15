/// combination_generator.rs
/// This module provides a generic combinator generator for the variables used in the configurations.
/// The set of available variables is passed to the constructor of the CombinationGenerator. Then, the parsed entries can be registered to be used in the combination generation using the `register_parsed_entry` method.
use crate::core::parsers::{
  ParserError,
  entry_parser::{EntrySegment, ParsedEntry, VariableSegment},
  multi_hashmap::LayeredHashMap,
  variable_parser::{self, BasicVar, CompleteVar, ListVar, MapKind, MapVar, PythonVar, Scalar},
};
use petgraph::graph::{DiGraph, NodeIndex};
use pyo3::types::PyAnyMethods;
use std::{
  collections::{HashMap, VecDeque},
  ffi::CString,
};

#[derive(Clone, Debug)]
enum MapKey {
  Constant(String),
  Variable(String),
}

/// Represents the DEFINITION of a dynamic node.
#[derive(Clone, Debug)]
enum DynamicNodeDef<'a> {
  List(&'a ListVar),
  Map(&'a MapVar, MapKey),
  Python(&'a PythonVar),
}

/// Represents the RUNTIME STATE of a dynamic node.
#[derive(Clone, Debug)]
struct NodeState {
  /// Current index in the list/map (always set to 0 for python/scalars)
  index: usize,
  /// Cached result of the current step to avoid re-computation
  cached_value: Option<Scalar>,
}

/// Represents a node in the dependency graph.
struct DynamicNode<'a> {
  def: DynamicNodeDef<'a>,
  state: NodeState,
}

impl NodeState {
  fn new() -> Self {
    Self {
      index: 0,
      cached_value: None,
    }
  }
}

/// The combination generator generates all combinations of variables that are used in the registered entries.
/// Entries can be registered using the `register_parsed_entry` method.
/// After registering all entries, call `try_iter` to get an iterator over all combinations.
/// The iterator is a standard Rust iterator, calling `next` will advance to the next combination.
/// Use `get_segment_value` on the returned object to retrieve the value of a variable segment for the current combination.
pub(crate) struct CombinationGenerator<'a> {
  /// The complete variable map, used to get variable definitions.
  variable_map: &'a LayeredHashMap<String, CompleteVar>,
  /// Maps variable name -> Graph Node Index. Used to quickly find variables from their name when evaluating entries.
  dynamic_vars: HashMap<String, NodeIndex>,
  /// Dependency Graph. The only nodes in the graph are dynamic variables (lists, maps, python). Scalars do not need to be in the graph, as they never change. Edges represent dependencies (A -> B means B depends on A).
  graph: DiGraph<DynamicNode<'a>, ()>,
  /// The cluster name, used for cluster-specific map lookups.
  cluster: String,
}

impl<'a> CombinationGenerator<'a> {
  pub fn new(variable_map: &'a LayeredHashMap<String, CompleteVar>, cluster: &str) -> Self {
    Self {
      variable_map,
      cluster: cluster.to_string(),
      dynamic_vars: HashMap::new(),
      graph: DiGraph::new(),
    }
  }

  /// Register all variable segments found in the entries. Variables must be defined in the variable map, otherwise an error is raised.
  pub fn register_parsed_entry(&mut self, entry: &ParsedEntry) -> Result<(), ParserError> {
    for segment in &entry.segments {
      if let EntrySegment::VariableSegment(vs) = segment {
        self.register_segment(vs)?;
      }
    }
    Ok(())
  }

  /// If the segment refers to a dynamic variable (list, map, python), register it. Registering means adding it to the graph and registering its dependencies recursively.
  fn register_segment(
    &mut self,
    segment: &VariableSegment,
  ) -> Result<Option<NodeIndex>, ParserError> {
    let name = segment.name();

    // Return early if already registered
    if let Some(&idx) = self.dynamic_vars.get(&name) {
      return Ok(Some(idx));
    }

    // Get the variable name, without the key part for maps
    let root_var_name = match segment {
      VariableSegment::Variable(n) => n.clone(),
      VariableSegment::ConstantMap { map_name, .. } => map_name.clone(),
      VariableSegment::VariableMap { map_name, .. } => map_name.clone(),
    };

    // 1. Resolve the variable from the variable map
    let var = self
      .variable_map
      .get(&root_var_name)
      .ok_or_else(|| ParserError::EvalError(format!("Variable '{}' not found", root_var_name)))?;

    // 2. Register based on type of segment and variable
    match segment {
      VariableSegment::Variable(_) => match var {
        CompleteVar::BasicVar(BasicVar::List(l)) => {
          Ok(Some(self.add_node(name, DynamicNodeDef::List(l))))
        }
        CompleteVar::Python(p) => self.add_python_node(name, p).map(Some),
        CompleteVar::Map(m) => match m.kind() {
          MapKind::Cluster => Ok(Some(self.add_node(
            name,
            DynamicNodeDef::Map(m, MapKey::Constant(self.cluster.clone())),
          ))),
          MapKind::Standard => Err(ParserError::EvalError(format!(
            "Map '{}' needs a key",
            name
          ))),
        },
        _ => Ok(None), // Scalars are static
      },
      VariableSegment::ConstantMap { map_name, key } => {
        let m = var
          .as_map()
          .ok_or_else(|| ParserError::EvalError(format!("'{}' is not a map", map_name)))?;
        Ok(Some(self.add_node(
          name,
          DynamicNodeDef::Map(m, MapKey::Constant(key.clone())),
        )))
      }
      VariableSegment::VariableMap { map_name, var_name } => {
        let m = var
          .as_map()
          .ok_or_else(|| ParserError::EvalError(format!("'{}' is not a map", map_name)))?;
        // Recursively register the key variable
        let key_idx = self.register_segment(&VariableSegment::Variable(var_name.clone()))?;

        let idx = self.add_node(
          name,
          DynamicNodeDef::Map(m, MapKey::Variable(var_name.clone())),
        );
        // If key is dynamic, add dependency edge (Key -> Map)
        if let Some(k_idx) = key_idx {
          self.graph.add_edge(k_idx, idx, ());
        }
        Ok(Some(idx))
      }
    }
  }

  fn add_node(&mut self, name: String, def: DynamicNodeDef<'a>) -> NodeIndex {
    let node = DynamicNode {
      def,
      state: NodeState::new(),
    };
    let idx = self.graph.add_node(node);
    self.dynamic_vars.insert(name, idx);
    idx
  }

  fn add_python_node(
    &mut self,
    name: String,
    var: &'a PythonVar,
  ) -> Result<NodeIndex, ParserError> {
    let idx = self.add_node(name, DynamicNodeDef::Python(var));
    // Register variables used in the python code and add dependency edges
    for seg in &var.code.segments {
      if let EntrySegment::VariableSegment(vs) = seg {
        if let Some(dep_idx) = self.register_segment(vs)? {
          self.graph.add_edge(dep_idx, idx, ());
        }
      }
    }
    Ok(idx)
  }

  pub fn try_iter(self) -> Result<CombinationIterator<'a>, ParserError> {
    CombinationIterator::new(self)
  }
}

enum State {
  NotStarted,
  InProgress,
  Finished,
}

pub(crate) struct CombinationIterator<'a> {
  generator: CombinationGenerator<'a>,
  // Vector of nodes sorted in topological order
  topo_nodes: Vec<NodeIndex>,
  state: State,
}

/// Generates all combinations of entries registered in the `CombinationGenerator` object given the variables stored in the combination generator.
impl<'a> CombinationIterator<'a> {
  pub fn new(generator: CombinationGenerator<'a>) -> Result<Self, ParserError> {
    let sorted_nodes = petgraph::algo::toposort(&generator.graph, None)
      .map_err(|_| ParserError::CyclicVariableDependency())?;
    Ok(Self {
      generator,
      topo_nodes: sorted_nodes,
      state: State::NotStarted,
    })
  }

  /// Get the value of a variable segment for the current iteration. Call .next() to advance to the next combination.
  pub fn get_segment_value(&self, segment: &VariableSegment) -> Result<Scalar, ParserError> {
    let name = segment.name();

    // 1. Try Dynamic
    if let Some(&idx) = self.generator.dynamic_vars.get(&name) {
      return self.resolve_node(idx);
    }

    // 2. Fallback Static
    let var = self
      .generator
      .variable_map
      .get(&name)
      .ok_or_else(|| ParserError::EvalError(format!("Variable '{}' not found", name)))?;

    match var {
      CompleteVar::BasicVar(BasicVar::Scalar(s)) => Ok(s.clone()),
      _ => Err(ParserError::EvalError(format!(
        "Variable '{}' is dynamic but not found in graph",
        name
      ))),
    }
  }

  /// Helper to resolve a dynamic node's value based on its current index and definition.
  /// The values of the nodes are computed lazily and cached
  fn resolve_node(&self, idx: NodeIndex) -> Result<Scalar, ParserError> {
    // Check cache (borrow checker workaround: get index first, then compute)
    let s = self.generator.graph.node_weight(idx).unwrap();
    let index = {
      // The node should always exist
      if let Some(val) = &s.state.cached_value {
        return Ok(val.clone());
      }
      s.state.index
    };

    // Compute value
    let value = match s.def {
      DynamicNodeDef::List(ref l) => l.get(index)?,
      DynamicNodeDef::Map(ref m, ref key_source) => {
        let key = match key_source {
          MapKey::Constant(k) => k,
          // Recursive call to resolve the variable key
          MapKey::Variable(v) => &self
            .get_segment_value(&VariableSegment::Variable(v.to_string()))?
            .to_string(),
        };

        let val_in_map = m.get(&key)?;
        match val_in_map {
          BasicVar::List(l) => l.get(index)?,
          BasicVar::Scalar(s) => s.clone(),
        }
      }
      DynamicNodeDef::Python(p) => {
        // Substitute variables in the python code, run the code and return the result
        let code = p.code.render(self)?;
        let code = CString::new(code)
          .map_err(|e| ParserError::EvalError(format!("Failed to convert code to CStr: {}", e)))?;
        pyo3::Python::attach(|py| {
          let result = py
            .eval(&code, None, None)
            .map_err(|e| ParserError::EvalError(format!("Python evaluation error: {}", e)))?;
          let output = result.extract::<String>().map_err(|e| {
            ParserError::EvalError(format!("Failed to extract Python eval result: {}", e))
          })?;

          Ok::<variable_parser::Scalar, ParserError>(Scalar::String(output))
        })?
      }
    };

    Ok(value)
  }

  /// Helper to determine how many variants the node generates.
  fn get_node_var_len(&mut self, idx: NodeIndex) -> Result<usize, ParserError> {
    let node_def = self.generator.graph[idx].def.clone();
    match node_def {
      DynamicNodeDef::List(l) => Ok(l.len()),
      DynamicNodeDef::Map(m, key_source) => {
        let key = match key_source {
          MapKey::Constant(k) => k,
          MapKey::Variable(v) => self
            .get_segment_value(&VariableSegment::Variable(v))?
            .to_string(),
        };
        match m.get(&key)? {
          BasicVar::List(l) => Ok(l.len()),
          BasicVar::Scalar(_) => Ok(1), // Scalars count as length 1
        }
      }
      DynamicNodeDef::Python(_) => Ok(1),
    }
  }

  /// The main function that generates the combinations. The topologically-sorted vector of nodes is iterated in reverse order.
  /// The algorithm that generates the combinations is similar to a binary ripple counter.
  /// The least-significant node (i.e. the one at the last position in the vector) is incremented first. The `index` attribute of the node's state is checked to see if all the variants of that node have been generated. If not, increment the index and perform a BFS starting from the changed node to clear the caches of all dependent nodes.
  /// If yes, reset the index to 0 and carry over to the next node. The same procedure is repeated on the next node.
  fn advance_iterator(&mut self) -> Option<()> {
    let mut advanced = false;

    for i in (0..self.topo_nodes.len()).rev() {
      let node_index = self.topo_nodes[i];
      // Gets the number of variants for the current node
      let len = self.get_node_var_len(node_index).ok()?;
      // Retrieves the actual node
      let node = self.generator.graph.node_weight_mut(node_index).unwrap();
      let state = &mut node.state;

      if state.index + 1 < len {
        state.index += 1;
        advanced = true;
        // Since we changed one index, the caches of all dependent nodes must be cleared.
        // Clear only dependents by performing a BFS from this node.
        let mut to_clear = VecDeque::new();
        to_clear.push_back(node_index);
        while let Some(current) = to_clear.pop_front() {
          // Invalidate cache
          if let Some(s) = self.generator.graph.node_weight_mut(current) {
            s.state.cached_value = None;
          }
          // Enqueue dependents
          for neighbor in self
            .generator
            .graph
            .neighbors_directed(current, petgraph::Direction::Outgoing)
          {
            to_clear.push_back(neighbor);
          }
        }
        break;
      } else {
        state.index = 0; // Reset and carry over
      }
    }

    if !advanced {
      self.state = State::Finished;
      return None;
    }

    Some(())
  }
}

impl<'a> Iterator for CombinationIterator<'a> {
  type Item = ();

  /// Advance to the next combination. Call .get_segment_value() to retrieve values. Returns None when all combinations have been generated.
  /// The case where there are no combinations is not handled, as we assume at least one combination exists (this would only in the case of an empty list).
  fn next(&mut self) -> Option<Self::Item> {
    match self.state {
      State::Finished => return None,
      State::NotStarted => {
        // Return early on the first iteration, as the indices are already at 0
        self.state = State::InProgress;
        return Some(());
      }
      State::InProgress => self.advance_iterator(),
    }
  }
}
