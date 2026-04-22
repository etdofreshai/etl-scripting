use std::path::PathBuf;
use std::process::Command;

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

#[test]
fn cli_parse_prints_ast_for_example_file() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let output = Command::new(binary)
        .arg("parse")
        .arg(example_path("hello_world.etl"))
        .output()
        .expect("parse command should run");

    assert!(
        output.status.success(),
        "parse command failed: {:?}",
        output
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("SourceFile"));
    assert!(stdout.contains("hello_world"));
}

#[test]
fn cli_check_reports_success_for_valid_example() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let output = Command::new(binary)
        .arg("check")
        .arg(example_path("hello_world.etl"))
        .output()
        .expect("check command should run");

    assert!(
        output.status.success(),
        "check command failed: {:?}",
        output
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("OK"));
}
