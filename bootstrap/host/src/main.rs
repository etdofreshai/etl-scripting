#![allow(dead_code)]

mod ast;
mod diagnostic;
mod interpreter;
mod ir;
mod lexer;
mod lir;
mod parser;
mod span;
mod token;
mod typecheck;

use std::env;
use std::fs;
use std::process;

fn print_help() {
    println!("etl-bootstrap-host");
    println!("usage:");
    println!("  etl parse <file.etl>");
    println!("  etl check <file.etl>");
    println!("  etl run <file.etl>");
    println!("  etl format <file.etl>");
    println!("  etl compile <file.etl> --to ir");
    println!("  etl compile <file.etl> --to lir");
    println!("  etl compile <file.etl> --to asm");
    println!("  etl compile <file.etl> --to native --target linux-x86_64");
}

fn read_source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        eprintln!("failed to read {path}: {error}");
        process::exit(1);
    })
}

fn parse_or_exit(path: &str) -> ast::SourceFile {
    let source = read_source(path);
    parser::parse_source(&source).unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(1);
    })
}

fn check_or_exit(path: &str) -> ast::SourceFile {
    let file = parse_or_exit(path);
    typecheck::validate_source_file(&file).unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(1);
    });
    file
}

fn compile_ir_or_exit(path: &str) {
    let file = check_or_exit(path);
    let program = ir::lower_source_file(&file);
    print!("{}", ir::render_program(&program));
}

fn compile_lir_or_exit(path: &str) {
    let file = check_or_exit(path);
    let ir_program = ir::lower_source_file(&file);
    let linear_program = lir::lower_program(&ir_program).unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(1);
    });
    print!("{}", lir::render_program(&linear_program));
}

fn compile_or_exit(args: &[String]) {
    if args.len() < 5 {
        eprintln!("compile requires a file path and --to <target>");
        process::exit(1);
    }

    let path = &args[2];
    if args[3] != "--to" {
        eprintln!("compile requires --to <target>");
        process::exit(1);
    }

    match args[4].as_str() {
        "ir" => compile_ir_or_exit(path),
        "lir" => compile_lir_or_exit(path),
        "asm" | "native" => println!("compile target not implemented yet: {}", args[4]),
        other => {
            eprintln!("unknown compile target: {other}");
            process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "parse" => {
            if args.len() < 3 {
                eprintln!("parse requires a file path");
                process::exit(1);
            }
            let file = parse_or_exit(&args[2]);
            println!("{:#?}", file);
        }
        "check" => {
            if args.len() < 3 {
                eprintln!("check requires a file path");
                process::exit(1);
            }
            let file = check_or_exit(&args[2]);
            println!("OK: {}", file.module_path.join("."));
        }
        "run" => {
            if args.len() < 3 {
                eprintln!("run requires a file path");
                process::exit(1);
            }
            let file = check_or_exit(&args[2]);
            let exit_code = interpreter::run_main(&file).unwrap_or_else(|error| {
                eprintln!("{error}");
                process::exit(1);
            });
            process::exit(exit_code as i32);
        }
        "format" => println!("format command not implemented yet"),
        "compile" => compile_or_exit(&args),
        _ => print_help(),
    }
}
