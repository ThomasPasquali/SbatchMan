use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use log::debug;
use crate::core::parsers::utils::{load_yaml_from_file, lookup_sequence, lookup_str, yaml_lookup};
use crate::core::parsers::variables::{parse_variables, Variable};
use crate::core::parsers::ParserError;

/// Push a file to the include list, checking for circular includes
fn push_file_to_include_list(
  file: &str,
  base_path: &Path,
  included_files: &mut Vec<PathBuf>,
  to_include: &mut Vec<PathBuf>,
) -> Result<(), ParserError> {
  let path = if Path::new(file).is_absolute() {
    PathBuf::from(file)
  } else {
    base_path
      .parent()
      .ok_or(ParserError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("Cannot determine parent directory of path {:?}", base_path),
      )))?
      .join(file)
  };
  let canonical_path = fs::canonicalize(&path)?;
  if included_files.contains(&canonical_path) {
    return Err(ParserError::CircularInclude(file.to_string()));
  }
  to_include.push(canonical_path);
  Ok(())
}

/// Recursively get variables from included YAML files
pub fn get_include_variables<'a>(root: &Path) -> Result<HashMap<String, Variable>, ParserError> {
  // Keep track of included files to prevent circular includes
  let mut included_files = vec!();
  // Start with the initial file
  let mut to_include = vec!(fs::canonicalize(root)?);
  // Final variables collection
  let mut variables = HashMap::new();
  
  // Process the include stack
  while let Some(current_path) = to_include.pop() {
    debug!("Loading included variables from file: {:?}", &current_path);

    let yaml = load_yaml_from_file(&current_path)?;
  
    // Single include
    if let Some(node) = yaml_lookup(&yaml, "include") {
      if let Some(file) = node.as_str() {
        push_file_to_include_list(&file, &current_path, &mut included_files, &mut to_include)?;
      } else if let Some(include_sequence) = node.as_sequence() {
        // Multiple includes. Last in list should be processed first, so that variables included from earlier files or deeper in the tree don't override earlier ones. Therefore we push to the stack from first to last.
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

    // After handling includes, parse variables from the current file
    if let Some(yaml_variables) = yaml_lookup(&yaml, "variables") {
      let new_variables = parse_variables(&yaml_variables)?;
      for (k, v) in new_variables {
        variables.entry(k).or_insert(v);
      }
    }
  }
  
  Ok(variables)
}
