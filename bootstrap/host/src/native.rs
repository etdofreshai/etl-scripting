use crate::lir::{LinearInstruction, LinearProgram};
use std::fmt::Write;

pub fn render_program(program: &LinearProgram, target: &str) -> Result<String, String> {
    match target {
        "linux-x86_64" => Ok(render_linux_x86_64(program)),
        other => Err(format!("unsupported native target: {other}")),
    }
}

fn render_linux_x86_64(program: &LinearProgram) -> String {
    let mut output = String::new();
    writeln!(&mut output, "target linux-x86_64").unwrap();
    writeln!(&mut output, "format elf64").unwrap();
    writeln!(&mut output, "default rel").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(&mut output, "section .text").unwrap();

    for callee in collect_external_callees(program) {
        writeln!(&mut output, "extern {callee}").unwrap();
    }

    if !program.functions.is_empty() {
        writeln!(&mut output).unwrap();
    }

    for function in &program.functions {
        writeln!(&mut output, "global {}", function.name).unwrap();
    }

    if !program.functions.is_empty() {
        writeln!(&mut output).unwrap();
    }

    for (index, function) in program.functions.iter().enumerate() {
        if index > 0 {
            writeln!(&mut output).unwrap();
        }

        writeln!(&mut output, "{}:", function.name).unwrap();
        writeln!(&mut output, "    push rbp").unwrap();
        writeln!(&mut output, "    mov rbp, rsp").unwrap();

        for instruction in &function.instructions {
            match instruction {
                LinearInstruction::Label(name) => writeln!(&mut output, "{}:", name).unwrap(),
                LinearInstruction::Return => {
                    writeln!(&mut output, "    mov rsp, rbp").unwrap();
                    writeln!(&mut output, "    pop rbp").unwrap();
                    writeln!(&mut output, "    ret").unwrap();
                }
                other => writeln!(&mut output, "    {}", render_instruction(other)).unwrap(),
            }
        }
    }

    output
}

fn collect_external_callees(program: &LinearProgram) -> Vec<String> {
    let mut callees = program
        .functions
        .iter()
        .flat_map(|function| function.instructions.iter())
        .filter_map(|instruction| match instruction {
            LinearInstruction::Call { callee, .. } => Some(callee.join(".")),
            _ => None,
        })
        .collect::<Vec<_>>();
    callees.sort();
    callees.dedup();
    callees
}

fn render_instruction(instruction: &LinearInstruction) -> String {
    match instruction {
        LinearInstruction::LoadInteger(value) => format!("push_int {value}"),
        LinearInstruction::LoadBoolean(value) => format!("push_bool {value}"),
        LinearInstruction::LoadText(value) => format!("push_text {value:?}"),
        LinearInstruction::LoadReference(path) => format!("load {}", path.join(".")),
        LinearInstruction::ConstructRecord {
            type_name,
            field_count,
        } => format!("construct_record {type_name}, {field_count}"),
        LinearInstruction::Call {
            callee,
            argument_count,
        } => format!("call {}, {}", callee.join("."), argument_count),
        LinearInstruction::Add => "add_pop".to_string(),
        LinearInstruction::Subtract => "sub_pop".to_string(),
        LinearInstruction::Multiply => "mul_pop".to_string(),
        LinearInstruction::Divide => "div_pop".to_string(),
        LinearInstruction::CompareEqual => "cmp_eq_pop".to_string(),
        LinearInstruction::CompareLess => "cmp_lt_pop".to_string(),
        LinearInstruction::CompareGreater => "cmp_gt_pop".to_string(),
        LinearInstruction::CompareLessEqual => "cmp_le_pop".to_string(),
        LinearInstruction::CompareGreaterEqual => "cmp_ge_pop".to_string(),
        LinearInstruction::LogicalAnd => "and_pop".to_string(),
        LinearInstruction::LogicalOr => "or_pop".to_string(),
        LinearInstruction::StoreLocal(name) => format!("store_local_pop {name}"),
        LinearInstruction::StoreReference(path) => format!("store_pop {}", path.join(".")),
        LinearInstruction::Jump(name) => format!("jmp {name}"),
        LinearInstruction::JumpIfFalse(name) => format!("jmp_if_false_pop {name}"),
        LinearInstruction::Pop => "pop".to_string(),
        LinearInstruction::Return | LinearInstruction::Label(_) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::render_program;
    use crate::ir::lower_source_file;
    use crate::lir::lower_program;
    use crate::parser::parse_source;
    use crate::typecheck::validate_source_file;

    #[test]
    fn renders_linux_x86_64_native_output() {
        let source = r#"module demo.native

define function main returns integer
    io.print_line("Hello")
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("target linux-x86_64"));
        assert!(native.contains("format elf64"));
        assert!(native.contains("default rel"));
        assert!(native.contains("section .text"));
        assert!(native.contains("global main"));
        assert!(native.contains("extern io.print_line"));
        assert!(native.contains("main:"));
        assert!(native.contains("    push rbp"));
        assert!(native.contains("    mov rbp, rsp"));
        assert!(native.contains("    call io.print_line, 1"));
        assert!(native.contains("    mov rsp, rbp"));
        assert!(native.contains("    pop rbp"));
        assert!(native.contains("    ret"));
    }
}
