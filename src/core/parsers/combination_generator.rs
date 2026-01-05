use crate::core::parsers::{
  ParserError,
  multi_hashmap::MultiHashMap,
  template_parser::{Template, TemplateSegment, VariableSegment},
  variables::{BasicVar, CompleteVar, ListVar, MapKind, MapVar, PythonVar, Scalar},
};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, VecDeque};

// --- 1. Graph Definitions (Immutable) ---

#[derive(Clone, Debug)]
enum MapKey {
  Constant(String),
  Variable(String),
}

/// Represents the DEFINITION of a dynamic node.
/// No mutable state (indices/caches) exists here.
#[derive(Clone, Debug)]
enum DynamicNodeDef<'a> {
  List(&'a ListVar),
  Map(&'a MapVar, MapKey),
  Python(&'a PythonVar),
}

// --- 2. Runtime State (Mutable) ---

#[derive(Clone, Debug)]
struct NodeState {
  /// Current index in the list/map (0 for python/scalars)
  index: usize,
  /// Cached result of the current step to avoid re-computation
  cached_value: Option<Scalar>,
}

impl NodeState {
  fn new() -> Self {
    Self {
      index: 0,
      cached_value: None,
    }
  }
}

pub(crate) struct CombinationGenerator<'a> {
  variable_map: &'a MultiHashMap<String, CompleteVar>,
  /// Maps variable name -> Graph Node Index
  dynamic_vars: HashMap<String, NodeIndex>,
  /// Dependency Graph (Definitions only)
  graph: DiGraph<DynamicNodeDef<'a>, ()>,
  cluster: String,
}

impl<'a> CombinationGenerator<'a> {
  pub fn new(variable_map: &'a MultiHashMap<String, CompleteVar>, cluster: &str) -> Self {
    Self {
      variable_map,
      cluster: cluster.to_string(),
      dynamic_vars: HashMap::new(),
      graph: DiGraph::new(),
    }
  }

  pub fn register_segments(&mut self, template: &Template) -> Result<(), ParserError> {
    for segment in &template.segments {
      if let TemplateSegment::VariableSegment(vs) = segment {
        self.register_variable(vs)?;
      }
    }
    Ok(())
  }

  /// Registers a variable and returns its NodeIndex if it is dynamic.
  fn register_variable(
    &mut self,
    segment: &VariableSegment,
  ) -> Result<Option<NodeIndex>, ParserError> {
    let name = segment.name();

    // Return early if already registered
    if let Some(&idx) = self.dynamic_vars.get(&name) {
      return Ok(Some(idx));
    }

    // 1. Resolve the definition from the Variable Map
    let var = self
      .variable_map
      .get(&name)
      .ok_or_else(|| ParserError::EvalError(format!("Variable '{}' not found", name)))?;

    // 2. Register based on type
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
        let key_idx = self.register_variable(&VariableSegment::Variable(var_name.clone()))?;

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
    let idx = self.graph.add_node(def);
    self.dynamic_vars.insert(name, idx);
    idx
  }

  fn add_python_node(
    &mut self,
    name: String,
    var: &'a PythonVar,
  ) -> Result<NodeIndex, ParserError> {
    let idx = self.add_node(name, DynamicNodeDef::Python(var));
    // Parse dependencies inside the python template
    for seg in &var.template.segments {
      if let TemplateSegment::VariableSegment(vs) = seg {
        if let Some(dep_idx) = self.register_variable(vs)? {
          self.graph.add_edge(dep_idx, idx, ());
        }
      }
    }
    Ok(idx)
  }

  pub fn try_iter(self) -> Result<CombinationIterator<'a>, ParserError> {
    let sorted_nodes = petgraph::algo::toposort(&self.graph, None)
      .map_err(|_| ParserError::CyclicVariableDependency())?;

    // Initialize state for every node
    let mut state = HashMap::new();
    for &idx in &sorted_nodes {
      state.insert(idx, NodeState::new());
    }

    Ok(CombinationIterator {
      generator: self,
      sorted_nodes,
      state,
      finished: false,
    })
  }
}

// --- 4. The Iterator (Runtime) ---

pub(crate) struct CombinationIterator<'a> {
  generator: CombinationGenerator<'a>,
  sorted_nodes: Vec<NodeIndex>,
  state: HashMap<NodeIndex, NodeState>,
  finished: bool,
}

impl<'a> CombinationIterator<'a> {
  /// Public API: Get the value of a segment for the current iteration
  pub fn get_segment_value(&mut self, segment: &VariableSegment) -> Result<Scalar, ParserError> {
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

  /// Internal: Pull-based resolution with caching
  fn resolve_node(&mut self, idx: NodeIndex) -> Result<Scalar, ParserError> {
    // Check cache (borrow checker workaround: get index first, then compute)
    let index = {
      let s = self.state.get(&idx).unwrap();
      if let Some(val) = &s.cached_value {
        return Ok(val.clone());
      }
      s.index
    };

    // Compute value
    let node_def = self.generator.graph[idx].clone();
    let value = match node_def {
      DynamicNodeDef::List(l) => l.get(index)?,
      DynamicNodeDef::Map(m, key_source) => {
        let key = match key_source {
          MapKey::Constant(k) => k,
          // Recursive call to resolve the variable key
          MapKey::Variable(v) => self
            .get_segment_value(&VariableSegment::Variable(v))?
            .to_string(),
        };

        let val_in_map = m.get(&key)?;
        match val_in_map {
          BasicVar::List(l) => l.get(index)?,
          BasicVar::Scalar(s) => s.clone(),
        }
      }
      DynamicNodeDef::Python(_p) => {
        // TODO: Execute python logic using self.get_segment_value to resolve inputs
        Scalar::String("python_result".into())
      }
    };

    // Update cache
    if let Some(s) = self.state.get_mut(&idx) {
      s.cached_value = Some(value.clone());
    }
    Ok(value)
  }

  /// Helper to determine how many items a node has in the current context
  fn get_node_len(&mut self, idx: NodeIndex) -> Result<usize, ParserError> {
    let node_def = self.generator.graph[idx].clone();
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
}

impl<'a> Iterator for CombinationIterator<'a> {
  type Item = ();

  fn next(&mut self) -> Option<Self::Item> {
    if self.finished {
      return None;
    }

    // 1. Advance Logic (Odometer)
    // Iterate independent vars first (or dependents, depending on desired order).
    // Reverse topological order is standard for "dependent-first" counting,
    // but for job matrices, we usually want independent vars to tick slowest.
    let mut advanced = false;

    for i in (0..self.sorted_nodes.len()).rev() {
      let idx = self.sorted_nodes[i];
      let len = self.get_node_len(idx).ok()?; // If error, stop iteration (simplification)
      let state = self.state.get_mut(&idx).unwrap();

      if state.index + 1 < len {
        state.index += 1;
        advanced = true;
        // Since we changed one index, the caches of all dependent nodes must be cleared.
        // Clear only dependents by performing a BFS from this node.
        let mut to_clear = VecDeque::new();
        to_clear.push_back(idx);
        while let Some(current) = to_clear.pop_front() {
          if let Some(s) = self.state.get_mut(&current) {
            s.cached_value = None;
          }
          for neighbor in
            self.generator.graph.neighbors_directed(current, petgraph::Direction::Outgoing)
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
      self.finished = true;
      return None;
    }

    Some(())
  }
}
