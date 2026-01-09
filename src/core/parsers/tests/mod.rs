use std::path::PathBuf;

mod variable_parser;
mod config_parser;
mod job_parser;

fn get_test_path(p: &str) -> PathBuf {
  PathBuf::from("src/core/parsers/tests/files").join(p)
}