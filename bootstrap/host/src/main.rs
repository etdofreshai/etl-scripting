#![allow(dead_code)]

mod ast;
mod diagnostic;
mod lexer;
mod parser;
mod span;
mod token;

use std::env;

fn print_help() {
    println!("etl-bootstrap-host");
    println!("usage:");
    println!("  etl parse <file.etl>");
    println!("  etl check <file.etl>");
    println!("  etl format <file.etl>");
    println!("  etl compile <file.etl> --to asm");
    println!("  etl compile <file.etl> --to native --target linux-x86_64");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "parse" => println!("parse command not implemented yet"),
        "check" => println!("check command not implemented yet"),
        "format" => println!("format command not implemented yet"),
        "compile" => println!("compile command not implemented yet"),
        _ => print_help(),
    }
}
