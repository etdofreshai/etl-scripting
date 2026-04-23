use crate::lir::{LinearFunction, LinearInstruction, LinearProgram};
use std::collections::BTreeMap;
use std::fmt::Write;

pub fn render_program(program: &LinearProgram, target: &str) -> Result<String, String> {
    match target {
        "linux-x86_64" => Ok(render_linux_x86_64(program)),
        other => Err(format!("unsupported native target: {other}")),
    }
}

fn render_linux_x86_64(program: &LinearProgram) -> String {
    let string_literals = collect_string_literals(program);
    let mut output = String::new();
    writeln!(&mut output, "target linux-x86_64").unwrap();
    writeln!(&mut output, "format elf64").unwrap();
    writeln!(&mut output, "default rel").unwrap();
    writeln!(&mut output).unwrap();

    if !string_literals.is_empty() {
        writeln!(&mut output, "section .rodata").unwrap();
        for (label, value) in &string_literals {
            writeln!(&mut output, "{label}: db {:?}, 0", value).unwrap();
        }
        writeln!(&mut output).unwrap();
    }

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

        let local_offsets = collect_local_offsets(function);
        writeln!(&mut output, "{}:", function.name).unwrap();
        writeln!(&mut output, "    push rbp").unwrap();
        writeln!(&mut output, "    mov rbp, rsp").unwrap();
        if !local_offsets.is_empty() {
            writeln!(&mut output, "    sub rsp, {}", local_offsets.len() * 8).unwrap();
        }

        let mut instruction_index = 0;
        while instruction_index < function.instructions.len() {
            match &function.instructions[instruction_index] {
                LinearInstruction::LoadInteger(value)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::Return)
                    ) =>
                {
                    writeln!(&mut output, "    mov rax, {value}").unwrap();
                }
                LinearInstruction::LoadInteger(value)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::StoreLocal(_))
                    ) =>
                {
                    if let Some(LinearInstruction::StoreLocal(name)) =
                        function.instructions.get(instruction_index + 1)
                    {
                        if let Some(offset) = local_offsets.get(name) {
                            writeln!(&mut output, "    mov qword [rbp-{offset}], {value}").unwrap();
                            instruction_index += 1;
                        }
                    }
                }
                LinearInstruction::LoadReference(path)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::Return)
                    ) && path.len() == 1 =>
                {
                    if let Some(offset) = local_offsets.get(&path[0]) {
                        writeln!(&mut output, "    mov rax, qword [rbp-{offset}]").unwrap();
                    } else {
                        writeln!(
                            &mut output,
                            "    {}",
                            render_instruction(&function.instructions[instruction_index])
                        )
                        .unwrap();
                    }
                }
                LinearInstruction::LoadReference(path)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::LoadInteger(_))
                    ) && path.len() == 1 =>
                {
                    if let Some(consumed) = try_render_local_compare_branch(
                        &mut output,
                        &local_offsets,
                        path,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_local_integer_update(
                        &mut output,
                        &local_offsets,
                        path,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else {
                        writeln!(
                            &mut output,
                            "    {}",
                            render_instruction(&function.instructions[instruction_index])
                        )
                        .unwrap();
                    }
                }
                LinearInstruction::LoadText(value)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::Call {
                            callee,
                            argument_count: 1,
                        }) if callee == &vec!["io".to_string(), "print_line".to_string()]
                    ) =>
                {
                    let label = string_label_for(&string_literals, value)
                        .expect("string label should exist");
                    writeln!(&mut output, "    lea rdi, [rel {label}]").unwrap();
                }
                LinearInstruction::Label(name) => writeln!(&mut output, "{}:", name).unwrap(),
                LinearInstruction::Return => {
                    writeln!(&mut output, "    mov rsp, rbp").unwrap();
                    writeln!(&mut output, "    pop rbp").unwrap();
                    writeln!(&mut output, "    ret").unwrap();
                }
                other => writeln!(&mut output, "    {}", render_instruction(other)).unwrap(),
            }
            instruction_index += 1;
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
            LinearInstruction::Call { callee, .. } => Some(native_symbol_name(&callee.join("."))),
            _ => None,
        })
        .collect::<Vec<_>>();
    callees.sort();
    callees.dedup();
    callees
}

fn collect_local_offsets(function: &LinearFunction) -> BTreeMap<String, usize> {
    let mut offsets = BTreeMap::new();
    for instruction in &function.instructions {
        match instruction {
            LinearInstruction::StoreLocal(name) => {
                let next_offset = (offsets.len() + 1) * 8;
                offsets.entry(name.clone()).or_insert(next_offset);
            }
            LinearInstruction::StoreReference(path) | LinearInstruction::LoadReference(path)
                if path.len() == 1 =>
            {
                let next_offset = (offsets.len() + 1) * 8;
                offsets.entry(path[0].clone()).or_insert(next_offset);
            }
            _ => {}
        }
    }
    offsets
}

fn inverse_jump_for_compare(compare: &LinearInstruction) -> Option<&'static str> {
    match compare {
        LinearInstruction::CompareLess => Some("jge"),
        LinearInstruction::CompareEqual => Some("jne"),
        LinearInstruction::CompareGreater => Some("jle"),
        LinearInstruction::CompareLessEqual => Some("jg"),
        LinearInstruction::CompareGreaterEqual => Some("jl"),
        _ => None,
    }
}

fn try_render_local_compare_branch(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    path: &[String],
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    if path.len() != 1 {
        return None;
    }

    let offset = *local_offsets.get(&path[0])?;
    let value = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };
    let compare = instructions.get(instruction_index + 2)?;
    let label = match instructions.get(instruction_index + 3) {
        Some(LinearInstruction::JumpIfFalse(label)) => label,
        _ => return None,
    };
    let jump = inverse_jump_for_compare(compare)?;

    writeln!(output, "    mov rax, qword [rbp-{offset}]").unwrap();
    writeln!(output, "    cmp rax, {value}").unwrap();
    writeln!(output, "    {jump} {label}").unwrap();
    Some(3)
}

fn arithmetic_mnemonic(instruction: &LinearInstruction) -> Option<&'static str> {
    match instruction {
        LinearInstruction::Add => Some("add"),
        LinearInstruction::Subtract => Some("sub"),
        LinearInstruction::Multiply => Some("imul"),
        _ => None,
    }
}

fn try_render_local_integer_update(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    path: &[String],
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    if path.len() != 1 {
        return None;
    }

    let offset = *local_offsets.get(&path[0])?;
    let value = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };
    let mnemonic = arithmetic_mnemonic(instructions.get(instruction_index + 2)?)?;

    match instructions.get(instruction_index + 3) {
        Some(LinearInstruction::StoreReference(target))
            if target.len() == 1 && target[0] == path[0] =>
        {
            writeln!(output, "    mov rax, qword [rbp-{offset}]").unwrap();
            writeln!(output, "    {mnemonic} rax, {value}").unwrap();
            writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
            Some(3)
        }
        _ => None,
    }
}

fn collect_string_literals(program: &LinearProgram) -> Vec<(String, String)> {
    let mut literals = Vec::new();
    for function in &program.functions {
        for instruction in &function.instructions {
            if let LinearInstruction::LoadText(value) = instruction {
                if literals.iter().all(|(_, existing)| existing != value) {
                    literals.push((format!("str_{}", literals.len()), value.clone()));
                }
            }
        }
    }
    literals
}

fn string_label_for<'a>(literals: &'a [(String, String)], value: &str) -> Option<&'a str> {
    literals
        .iter()
        .find(|(_, literal)| literal == value)
        .map(|(label, _)| label.as_str())
}

fn native_symbol_name(name: &str) -> String {
    name.replace('.', "_")
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
        LinearInstruction::Call { callee, .. } => {
            format!("call {}", native_symbol_name(&callee.join(".")))
        }
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
    use super::{inverse_jump_for_compare, render_program};
    use crate::ir::lower_source_file;
    use crate::lir::{lower_program, LinearInstruction};
    use crate::parser::parse_source;
    use crate::typecheck::validate_source_file;

    #[test]
    fn renders_linux_x86_64_native_output() {
        let source = r#"module demo.native

define function helper returns integer
    return 7

define function main returns integer
    io.print_line("Hello from ETL")
    return helper()
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
        assert!(native.contains("section .rodata"));
        assert!(native.contains("str_0: db \"Hello from ETL\", 0"));
        assert!(native.contains("section .text"));
        assert!(native.contains("global main"));
        assert!(native.contains("global helper"));
        assert!(native.contains("extern io_print_line"));
        assert!(native.contains("helper:"));
        assert!(native.contains("main:"));
        assert!(native.contains("    lea rdi, [rel str_0]"));
        assert!(native.contains("    call io_print_line"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("call helper, 0"));
        assert!(!native.contains("call io.print_line, 1"));
        assert!(native.contains("    mov rax, 7"));
        assert!(native.contains("    mov rsp, rbp"));
        assert!(native.contains("    pop rbp"));
        assert!(native.contains("    ret"));
    }

    #[test]
    fn lowers_integer_local_updates_into_stack_slots() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 1
    set score to score + 2
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 1"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    add rax, 2"));
        assert!(native.contains("    mov qword [rbp-8], rax"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(!native.contains("store_local_pop score"));
        assert!(!native.contains("store_pop score"));
        assert!(!native.contains("add_pop"));
    }

    #[test]
    fn lowers_integer_local_subtraction_updates_into_stack_slots() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 5
    set score to score - 2
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    sub rax, 2"));
        assert!(native.contains("    mov qword [rbp-8], rax"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(!native.contains("store_pop score"));
        assert!(!native.contains("sub_pop"));
    }

    #[test]
    fn lowers_integer_local_comparisons_into_cmp_and_jump() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 1
    if score < 3
        return 1
    else
        return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 1"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, 3"));
        assert!(native.contains("    jge main_if_else_0"));
        assert!(!native.contains("cmp_lt_pop"));
        assert!(!native.contains("jmp_if_false_pop main_if_else_0"));
    }

    #[test]
    fn lowers_integer_equality_comparisons_into_cmp_and_jump() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 1
    if score == 1
        return 1
    else
        return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, 1"));
        assert!(native.contains("    jne main_if_else_0"));
        assert!(!native.contains("cmp_eq_pop"));
        assert!(!native.contains("jmp_if_false_pop main_if_else_0"));
    }

    #[test]
    fn lowers_integer_less_equal_comparisons_into_cmp_and_jump() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 1
    if score <= 3
        return 1
    else
        return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, 3"));
        assert!(native.contains("    jg main_if_else_0"));
        assert!(!native.contains("cmp_le_pop"));
        assert!(!native.contains("jmp_if_false_pop main_if_else_0"));
    }

    #[test]
    fn lowers_integer_greater_comparisons_into_cmp_and_jump() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 5
    if score > 3
        return 1
    else
        return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, 3"));
        assert!(native.contains("    jle main_if_else_0"));
        assert!(!native.contains("cmp_gt_pop"));
        assert!(!native.contains("jmp_if_false_pop main_if_else_0"));
    }

    #[test]
    fn lowers_integer_greater_equal_comparisons_into_cmp_and_jump() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 5
    if score >= 3
        return 1
    else
        return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, 3"));
        assert!(native.contains("    jl main_if_else_0"));
        assert!(!native.contains("cmp_ge_pop"));
        assert!(!native.contains("jmp_if_false_pop main_if_else_0"));
    }

    #[test]
    fn maps_compare_ops_to_inverse_jump_mnemonics() {
        assert_eq!(
            inverse_jump_for_compare(&LinearInstruction::CompareLess),
            Some("jge")
        );
        assert_eq!(
            inverse_jump_for_compare(&LinearInstruction::CompareEqual),
            Some("jne")
        );
        assert_eq!(
            inverse_jump_for_compare(&LinearInstruction::CompareGreater),
            Some("jle")
        );
        assert_eq!(
            inverse_jump_for_compare(&LinearInstruction::CompareLessEqual),
            Some("jg")
        );
        assert_eq!(
            inverse_jump_for_compare(&LinearInstruction::CompareGreaterEqual),
            Some("jl")
        );
        assert_eq!(inverse_jump_for_compare(&LinearInstruction::Add), None);
    }
}
