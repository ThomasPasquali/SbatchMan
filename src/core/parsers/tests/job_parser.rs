use crate::core::parsers::job_parser::parse_jobs_from_file;
use super::get_test_path;

#[test]
fn test_parse_jobs_complex() {
    let path = get_test_path("test_jobs.yaml");
    let jobs = parse_jobs_from_file(&path, "clusterA").expect("Failed to parse jobs");
    
    // We expect 7 jobs total:
    // Block 1: 2 (list_var) * 2 (job_var) = 4 jobs
    // Block 2: 
    //   Variant 1: uses sub_var (2 values) -> 2 jobs
    //   Variant 2: does NOT use sub_var -> 1 job (unused vars are not iterated)
    assert_eq!(jobs.len(), 7);

    // --- Block 1 Checks ---
    // Template: name: "job_{{list_var}}_{{job_var}}"
    // Variants: list_var=[A, B], job_var=[1, 2]
    
    let job_a_1 = jobs.iter().find(|j| j.job_name == "job_A_1").expect("job_A_1 not found");
    // command: "cmd {{map_var[list_var]}} {{job_var}} {{cluster_var}}"
    // map_var[A] = mapped_A. job_var = 1. cluster_var = valA (for clusterA).
    assert_eq!(job_a_1.command, "cmd mapped_A 1 valA");
    assert_eq!(job_a_1.config_name, "config_valA");
    assert_eq!(job_a_1.preprocess.as_deref(), Some("pre value1"));
    assert_eq!(job_a_1.postprocess.as_deref(), Some("post"));

    let job_b_2 = jobs.iter().find(|j| j.job_name == "job_B_2").expect("job_B_2 not found");
    assert_eq!(job_b_2.command, "cmd mapped_B 2 valA");
    assert_eq!(job_b_2.config_name, "config_valA");

    // --- Block 2 Checks ---
    // Outer variables: sub_var = [x, y]
    // Variant 1: name "v1_{{sub_var}}", command "echo {{sub_var}}"
    // Variant 2: name "v2", command "echo v2"

    // Variant 1 generated jobs
    let v1_x = jobs.iter().find(|j| j.job_name == "v1_x").expect("v1_x not found");
    assert_eq!(v1_x.command, "echo x");
    assert_eq!(v1_x.config_name, "default"); // inherited

    let v1_y = jobs.iter().find(|j| j.job_name == "v1_y").expect("v1_y not found");
    assert_eq!(v1_y.command, "echo y");

    // Variant 2 generated jobs (single job "v2" because unused variables are skipped)
    let v2_count = jobs.iter().filter(|j| j.job_name == "v2").count();
    assert_eq!(v2_count, 1);
    
    let v2 = jobs.iter().find(|j| j.job_name == "v2").expect("v2 not found");
    assert_eq!(v2.command, "echo v2");
    assert_eq!(v2.config_name, "default");
}

#[test]
fn test_parse_jobs_cluster_b_variations() {
    // Testing clusterB which affects `cluster_var` -> "valB"
    let path = get_test_path("test_jobs.yaml");
    let jobs = parse_jobs_from_file(&path, "clusterB").expect("Failed to parse jobs");

    // Check cluster variable substitution
    let job_a_1 = jobs.iter().find(|j| j.job_name == "job_A_1").expect("job_A_1 not found");
    assert_eq!(job_a_1.command, "cmd mapped_A 1 valB");
    assert_eq!(job_a_1.config_name, "config_valB");
}

#[test]
fn test_parse_jobs_missing_file() {
    let path = get_test_path("non_existent.yaml");
    let result = parse_jobs_from_file(&path, "clusterA");
    assert!(result.is_err());
}
