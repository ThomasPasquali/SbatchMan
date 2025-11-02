use std::fs;
use std::path::{Path, PathBuf};

use crate::core::parsers::ParserError;
use crate::core::parsers::utils::{load_yaml_from_file, lookup_mapping, yaml_lookup};
use crate::core::parsers::variables::{Variable, parse_variables};
use hashlink::LinkedHashMap;
use log::debug;

/// Push a file to the include list, checking for circular includes
fn push_file_to_include_list(
  file: &str,
  file_path: &Path,
  included_files: &mut Vec<PathBuf>,
  to_include: &mut Vec<PathBuf>,
) -> Result<(), ParserError> {
  let path = if Path::new(file).is_absolute() {
    // Absolute path
    PathBuf::from(file)
  } else {
    // A relative path was provided, resolve the path relative to the file path
    file_path
      .parent()
      .ok_or(ParserError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("Cannot determine parent directory of file {:?}", file_path),
      )))?
      .join(file)
  };
  let canonical_path = fs::canonicalize(&path)?;
  // If file is already in the list of included files, we have a circular include, raise an error
  if included_files.contains(&canonical_path) {
    return Err(ParserError::CircularInclude(file.to_string()));
  }
  to_include.push(canonical_path);
  Ok(())
}

/// Collect all variables from included YAML files. The function performs a depth-first traversal of includes. Variables from later includes override those earlier ones.
pub fn get_include_variables<'a>(root: &Path) -> Result<LinkedHashMap<String, Variable>, ParserError> {
  // Keep track of included files to prevent circular includes
  let mut included_files = vec![];
  // Start with the initial file
  let mut to_include = vec![fs::canonicalize(root)?];
  // Final variables collection
  let mut variables = LinkedHashMap::new();

  // Process the include stack. Variables from this file are processed first. Then, variables from included files are processed, but do not override variables that have been already inserted.
  while let Some(current_path) = to_include.pop() {
    debug!("Loading included variables from file: {:?}", &current_path);

    let yaml = load_yaml_from_file(&current_path)?;

    // Parse variables from the current file
    if let Ok(yaml_variables) = lookup_mapping(&yaml, "variables") {
      let new_variables = parse_variables(&yaml_variables)?;
      // Merge new variables, without overriding existing ones
      for (k, v) in new_variables {
        variables.entry(k).or_insert(v);
      }
    }

    if let Some(node) = yaml_lookup(&yaml, "include") {
      if let Some(file) = node.as_str() {
        // Single include
        push_file_to_include_list(&file, &current_path, &mut included_files, &mut to_include)?;
      } else if let Some(include_sequence) = node.as_sequence() {
        // Multiple includes. Push from first to last, so that last will be processed first (LIFO)
        for it in include_sequence.iter() {
          if let Some(file) = it.as_str() {
            push_file_to_include_list(file, &current_path, &mut included_files, &mut to_include)?;
          } else {
            return Err(ParserError::IncludeWrongType(format!("{:?}", it)));
          }
        }
      } else {
        return Err(ParserError::IncludeWrongType(format!("{:?}", node)));
      }
    }
    included_files.push(fs::canonicalize(current_path)?);
  }

  Ok(variables)
}
