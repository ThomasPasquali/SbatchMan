procedure (configurations):
parse top-level variables and add them to the variable multi-hashmap
parse top-level config entries and add them to the config multi-hashmap
for each cluster in the clusters configuration:
  parse cluster-level variables and add them to the multi-hashmap
  parse cluster-level config entries and add them to the config multi-hashmap
  for each config in the cluster:
    parse cluster-level variables and add them to the multi-hashmap
    parse cluster-level config entries and add them to the config multi-hashmap
    iterate over config multi-hashmap to build a dependency graph of variables and config entries
    topologically sort the dependency graph
    run combination generation procedure

combination generation procedure:
from last to first node in topological order:
  increment current_index
  if current_index overflows:
    reset current_index to 0
  else:
    for each dependent node in BFS order:
      if node is a VariableNode:
        re-evaluate variable
      else if node is a ConfigEntryNode:
        re-evaluate config entry and write it to final config object
    break

enum TemplateSegment {
  Literal(String),
  Variable(VariableNode),
}

struct ParsedTemplate {
  string: Vec<TemplateSegment>,

  print(self) {
    String result = ""
    for segment in self.string {
      match segment {
        Literal(s) => result += s,
        Variable(v) => result += v.values[v.current_index],
      }
    }
  }
}

HashMap<String, ConfigEntryNode> config_entries // map of configuration entries by name, iterable

// Node representing a configuration entry in the dependency graph
struct ConfigEntryNode {
  name: String,
  value: ParsedTemplate,
  type: Param | Env,
}

// Node representing a variable in the dependency graph.
struct VariableNode {
  name: String,
  current_index: usize,
  values: Vec<String>,
  original_variable: CompleteVar,
}

// Graph of variables and configuration entries used, where edges represent dependencies
// A dependency is created in the following cases:
// - index of a map variable is another variable
// - python expression uses other variables
// - a config entry value uses variables
GraphMap<VariableNode | ConfigEntryNode, (), Directed> dependency_graph;

// Before starting generating combinations, sort the graph topologically
// When a variable's current_index is incremented, a BFS is performed to find all dependent nodes and re-evaluate them