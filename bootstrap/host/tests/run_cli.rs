use std::path::PathBuf;
use std::process::Command;

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

#[test]
fn cli_run_executes_hello_world() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let output = Command::new(binary)
        .arg("run")
        .arg(example_path("hello_world.etl"))
        .output()
        .expect("run command should execute");

    assert!(output.status.success(), "run command failed: {:?}", output);

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Hello from ETL"));
}

#[test]
fn cli_run_returns_nonzero_exit_code_for_unknown_types() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let path = std::env::temp_dir().join("etl-run-invalid-type.etl");
    std::fs::write(
        &path,
        "module broken.types\n\ndefine function main returns mystery_type\n    return 0\n",
    )
    .expect("temp source should be written");

    let output = Command::new(binary)
        .arg("run")
        .arg(&path)
        .output()
        .expect("run command should execute");

    assert!(!output.status.success(), "run command should fail");
}
