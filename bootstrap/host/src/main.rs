#![allow(dead_code)]

mod ast;
mod diagnostic;
mod lexer;
mod parser;
mod span;
mod token;

use std::env;
use std::fs;
use std::process;

fn print_help() {
    println!("etl-bootstrap-host");
    println!("usage:");
    println!("  etl parse <file.etl>");
    println!("  etl check <file.etl>");
    println!("  etl format <file.etl>");
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
            let file = parse_or_exit(&args[2]);
            println!("OK: {}", file.module_path.join("."));
        }
        "format" => println!("format command not implemented yet"),
        "compile" => println!("compile command not implemented yet"),
        _ => print_help(),
    }
}
