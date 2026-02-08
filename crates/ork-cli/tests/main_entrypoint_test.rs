use std::process::Command;

#[test]
fn test_ork_binary_help_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_ork"))
        .arg("--help")
        .output()
        .expect("run ork --help");
    assert!(output.status.success(), "stdout: {:?}", output.stdout);
}

#[test]
fn test_ork_binary_run_workflow_missing_file_fails_before_db_connect() {
    let output = Command::new(env!("CARGO_BIN_EXE_ork"))
        .args([
            "run-workflow",
            "--file",
            "/tmp/does-not-exist-workflow.yaml",
            "--api-url",
            "http://127.0.0.1:1",
        ])
        .output()
        .expect("run ork run-workflow");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Missing workflow file"),
        "stderr was: {stderr}"
    );
}
