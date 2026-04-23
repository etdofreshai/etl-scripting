use crate::lir::{LinearInstruction, LinearProgram};
use std::fmt::Write;

pub fn render_program(program: &LinearProgram) -> String {
    let function_return_types = collect_function_return_types(program);
    let mut output = String::new();
    writeln!(&mut output, "section .text").unwrap();

    for callee in collect_external_callees(program) {
        writeln!(&mut output, "extern {callee}").unwrap();
    }

    if !program.functions.is_empty() {
        output.push('\n');
    }

    for function in &program.functions {
        writeln!(&mut output, "global {}", function.name).unwrap();
    }

    if !program.functions.is_empty() {
        output.push('\n');
    }

    for (index, function) in program.functions.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }

        writeln!(&mut output, "{}:", function.name).unwrap();
        for instruction in &function.instructions {
            match instruction {
                LinearInstruction::Label(name) => {
                    writeln!(&mut output, "{}:", name).unwrap();
                }
                other => {
                    writeln!(
                        &mut output,
                        "    {}",
                        render_instruction(other, &function.return_type, &function_return_types)
                    )
                    .unwrap();
                }
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

fn collect_function_return_types(
    program: &LinearProgram,
) -> std::collections::HashMap<String, String> {
    let mut return_types = std::collections::HashMap::from([
        ("io.print_line".to_string(), "void".to_string()),
        (
            "random.from_seed".to_string(),
            "standard.random.generator".to_string(),
        ),
        ("random.next_integer".to_string(), "integer".to_string()),
        ("event.push_hit".to_string(), "void".to_string()),
    ]);

    for function in &program.functions {
        return_types.insert(function.name.clone(), function.return_type.clone());
    }

    return_types
}

fn render_instruction(
    instruction: &LinearInstruction,
    current_function_return_type: &str,
    function_return_types: &std::collections::HashMap<String, String>,
) -> String {
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
        } => {
            let callee_name = callee.join(".");
            let opcode = match function_return_types.get(&callee_name).map(String::as_str) {
                Some("void") => "call_void",
                _ => "call_value",
            };
            format!("{opcode} {callee_name}, {argument_count}")
        }
        LinearInstruction::Add => "add".to_string(),
        LinearInstruction::Subtract => "sub".to_string(),
        LinearInstruction::Multiply => "mul".to_string(),
        LinearInstruction::Divide => "div".to_string(),
        LinearInstruction::CompareEqual => "cmp_eq".to_string(),
        LinearInstruction::CompareLess => "cmp_lt".to_string(),
        LinearInstruction::CompareGreater => "cmp_gt".to_string(),
        LinearInstruction::CompareLessEqual => "cmp_le".to_string(),
        LinearInstruction::CompareGreaterEqual => "cmp_ge".to_string(),
        LinearInstruction::LogicalAnd => "and".to_string(),
        LinearInstruction::LogicalOr => "or".to_string(),
        LinearInstruction::StoreLocal(name) => format!("store_local_pop {name}"),
        LinearInstruction::StoreReference(path) => format!("store_pop {}", path.join(".")),
        LinearInstruction::Jump(name) => format!("jmp {name}"),
        LinearInstruction::JumpIfFalse(name) => format!("jmp_if_false_pop {name}"),
        LinearInstruction::Pop => "pop".to_string(),
        LinearInstruction::Return => match current_function_return_type {
            "void" => "return_void".to_string(),
            _ => "return_value".to_string(),
        },
        LinearInstruction::Label(_) => unreachable!("labels are rendered separately"),
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
    fn renders_linear_program_as_textual_assembly() {
        let source = r#"module demo.asm

define function main takes ready as boolean returns integer
    if ready
        return 1
    else
        return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let asm = render_program(&linear);

        assert!(asm.contains("section .text"));
        assert!(asm.contains("global main"));
        assert!(asm.contains("main:"));
        assert!(asm.contains("    load ready"));
        assert!(asm.contains("    jmp_if_false_pop main_if_else_0"));
        assert!(asm.contains("main_if_else_0:"));
        assert!(asm.contains("    return_value"));
    }

    #[test]
    fn renders_builtin_calls_as_extern_declarations() {
        let source = r#"module demo.asm

define function main takes seed as integer returns integer
    io.print_line("Hello")
    let generator be random.from_seed(seed)
    return random.next_integer(generator, 1, 10)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let asm = render_program(&linear);

        assert!(asm.contains("extern io.print_line"));
        assert!(asm.contains("extern random.from_seed"));
        assert!(asm.contains("extern random.next_integer"));
    }

    #[test]
    fn renders_call_conventions_for_void_and_value_calls() {
        let source = r#"module demo.asm

define function make_number returns integer
    return 7

define function main returns integer
    io.print_line("Hello")
    return make_number()
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let asm = render_program(&linear);

        assert!(asm.contains("    call_void io.print_line, 1"));
        assert!(asm.contains("    call_value make_number, 0"));
    }

    #[test]
    fn renders_store_and_return_stack_effects_explicitly() {
        let source = r#"module demo.asm

define function main returns integer
    mutable score as integer be 1
    set score to score + 2
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let asm = render_program(&linear);

        assert!(asm.contains("    store_local_pop score"));
        assert!(asm.contains("    store_pop score"));
        assert!(asm.contains("    return_value"));
    }
}
