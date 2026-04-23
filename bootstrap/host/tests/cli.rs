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

#[test]
fn cli_check_rejects_unknown_types() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let path = std::env::temp_dir().join("etl-invalid-type.etl");
    std::fs::write(
        &path,
        "module broken.types\n\ndefine function broken takes value as mystery_type returns integer\n    return 0\n",
    )
    .expect("temp source should be written");

    let output = Command::new(binary)
        .arg("check")
        .arg(&path)
        .output()
        .expect("check command should run");

    assert!(!output.status.success(), "check command should fail");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unknown type: mystery_type"));
}

#[test]
fn cli_compile_to_ir_prints_lowered_program() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let output = Command::new(binary)
        .arg("compile")
        .arg(example_path("hello_world.etl"))
        .arg("--to")
        .arg("ir")
        .output()
        .expect("compile command should run");

    assert!(
        output.status.success(),
        "compile command failed: {:?}",
        output
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("module demo.hello_world"));
    assert!(stdout.contains("fn main() -> integer"));
    assert!(stdout.contains("expr io.print_line(\"Hello from ETL\")"));
    assert!(stdout.contains("return 0"));
}

#[test]
fn cli_compile_to_lir_prints_linear_lowering() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let output = Command::new(binary)
        .arg("compile")
        .arg(example_path("hello_world.etl"))
        .arg("--to")
        .arg("lir")
        .output()
        .expect("compile command should run");

    assert!(
        output.status.success(),
        "compile command failed: {:?}",
        output
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("fn main:"));
    assert!(stdout.contains("call io.print_line 1"));
    assert!(!stdout.contains("pop"));
    assert!(stdout.contains("load_integer 0"));
    assert!(stdout.contains("return"));
}

#[test]
fn cli_compile_to_asm_prints_textual_assembly() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let output = Command::new(binary)
        .arg("compile")
        .arg(example_path("hello_world.etl"))
        .arg("--to")
        .arg("asm")
        .output()
        .expect("compile command should run");

    assert!(
        output.status.success(),
        "compile command failed: {:?}",
        output
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("section .text"));
    assert!(stdout.contains("global main"));
    assert!(stdout.contains("main:"));
    assert!(stdout.contains("    call_void io.print_line, 1"));
    assert!(stdout.contains("    return_value"));
}

#[test]
fn cli_compile_to_native_linux_x86_64_prints_targeted_backend_output() {
    let binary = env!("CARGO_BIN_EXE_etl-bootstrap-host");
    let output = Command::new(binary)
        .arg("compile")
        .arg(example_path("hello_world.etl"))
        .arg("--to")
        .arg("native")
        .arg("--target")
        .arg("linux-x86_64")
        .output()
        .expect("compile command should run");

    assert!(
        output.status.success(),
        "compile command failed: {:?}",
        output
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("target linux-x86_64"));
    assert!(stdout.contains("format elf64"));
    assert!(stdout.contains("default rel"));
    assert!(stdout.contains("section .text"));
    assert!(stdout.contains("global main"));
    assert!(stdout.contains("    push rbp"));
    assert!(stdout.contains("    mov rbp, rsp"));
    assert!(stdout.contains("    mov rax, 0"));
    assert!(!stdout.contains("push_int 0"));
    assert!(stdout.contains("    ret"));
}
