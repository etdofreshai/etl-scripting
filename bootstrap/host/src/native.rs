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
    let user_functions = collect_user_function_names(program);
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
        render_parameter_prologue(&mut output, function, &local_offsets);

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
                LinearInstruction::LoadInteger(_)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::Call { .. })
                    ) =>
                {
                    if let Some(consumed) = try_render_integer_immediate_call(
                        &mut output,
                        &user_functions,
                        &local_offsets,
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
                LinearInstruction::LoadInteger(_)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::LoadInteger(_))
                    ) =>
                {
                    if let Some(consumed) = try_render_integer_immediate_call(
                        &mut output,
                        &user_functions,
                        &local_offsets,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_integer_local_initializer(
                        &mut output,
                        &local_offsets,
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
                LinearInstruction::LoadInteger(_)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::LoadReference(path)) if path.len() == 1
                    ) && matches!(
                        function.instructions.get(instruction_index + 2),
                        Some(LinearInstruction::Call { .. })
                    ) =>
                {
                    if let Some(consumed) = try_render_simple_user_call(
                        &mut output,
                        &user_functions,
                        &local_offsets,
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
                LinearInstruction::LoadInteger(_)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::LoadReference(path)) if path.len() == 1
                    ) =>
                {
                    if let Some(consumed) = try_render_commuted_local_integer_update(
                        &mut output,
                        &local_offsets,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_immediate_local_compare_return(
                        &mut output,
                        &local_offsets,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_immediate_local_integer_update(
                        &mut output,
                        &local_offsets,
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
                LinearInstruction::LoadReference(path)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::LoadReference(other_path)) if other_path.len() == 1
                    ) && path.len() == 1 =>
                {
                    if let Some(consumed) = try_render_simple_user_call(
                        &mut output,
                        &user_functions,
                        &local_offsets,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_local_compare_branch(
                        &mut output,
                        &local_offsets,
                        path,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_local_binary_return(
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
                LinearInstruction::LoadReference(path)
                    if matches!(
                        function.instructions.get(instruction_index + 1),
                        Some(LinearInstruction::Call {
                            argument_count: 1,
                            ..
                        })
                    ) && path.len() == 1 =>
                {
                    if let Some(consumed) = try_render_single_local_call(
                        &mut output,
                        &user_functions,
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
                    if let Some(consumed) = try_render_local_compute_call(
                        &mut output,
                        &user_functions,
                        &local_offsets,
                        path,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_local_compare_branch(
                        &mut output,
                        &local_offsets,
                        path,
                        &function.instructions,
                        instruction_index,
                    ) {
                        instruction_index += consumed;
                    } else if let Some(consumed) = try_render_local_compare_return(
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

fn collect_user_function_names(program: &LinearProgram) -> Vec<String> {
    program
        .functions
        .iter()
        .map(|function| function.name.clone())
        .collect()
}

fn render_parameter_prologue(
    output: &mut String,
    function: &LinearFunction,
    local_offsets: &BTreeMap<String, usize>,
) {
    for (index, parameter_name) in function.parameter_names.iter().enumerate() {
        if let Some(offset) = local_offsets.get(parameter_name) {
            if let Some(register) = integer_argument_register(index) {
                writeln!(output, "    mov qword [rbp-{offset}], {register}").unwrap();
            } else {
                let stack_offset = 16 + (index - 6) * 8;
                writeln!(output, "    mov rax, qword [rbp+{stack_offset}]").unwrap();
                writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
            }
        }
    }
}

fn integer_argument_register(index: usize) -> Option<&'static str> {
    match index {
        0 => Some("rdi"),
        1 => Some("rsi"),
        2 => Some("rdx"),
        3 => Some("rcx"),
        4 => Some("r8"),
        5 => Some("r9"),
        _ => None,
    }
}

fn collect_local_offsets(function: &LinearFunction) -> BTreeMap<String, usize> {
    let mut offsets = BTreeMap::new();
    for parameter_name in &function.parameter_names {
        let next_offset = (offsets.len() + 1) * 8;
        offsets.entry(parameter_name.clone()).or_insert(next_offset);
    }
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

fn setcc_for_compare(compare: &LinearInstruction) -> Option<&'static str> {
    match compare {
        LinearInstruction::CompareLess => Some("setl"),
        LinearInstruction::CompareEqual => Some("sete"),
        LinearInstruction::CompareGreater => Some("setg"),
        LinearInstruction::CompareLessEqual => Some("setle"),
        LinearInstruction::CompareGreaterEqual => Some("setge"),
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

    let left_offset = *local_offsets.get(&path[0])?;
    let (compare, label, right_operand, consumed) = match (
        instructions.get(instruction_index + 1),
        instructions.get(instruction_index + 2),
        instructions.get(instruction_index + 3),
    ) {
        (
            Some(LinearInstruction::LoadInteger(value)),
            Some(compare),
            Some(LinearInstruction::JumpIfFalse(label)),
        ) => (compare, label, value.to_string(), 3),
        (
            Some(LinearInstruction::LoadReference(other_path)),
            Some(compare),
            Some(LinearInstruction::JumpIfFalse(label)),
        ) if other_path.len() == 1 => {
            let right_offset = *local_offsets.get(&other_path[0])?;
            (compare, label, format!("qword [rbp-{right_offset}]"), 3)
        }
        _ => return None,
    };
    let jump = inverse_jump_for_compare(compare)?;

    writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
    writeln!(output, "    cmp rax, {right_operand}").unwrap();
    writeln!(output, "    {jump} {label}").unwrap();
    Some(consumed)
}

fn try_render_local_binary_return(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    path: &[String],
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let left_offset = *local_offsets.get(&path[0])?;
    let right_path = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadReference(path)) if path.len() == 1 => path,
        _ => return None,
    };
    let right_offset = *local_offsets.get(&right_path[0])?;

    match (
        instructions.get(instruction_index + 2)?,
        instructions.get(instruction_index + 3)?,
    ) {
        (LinearInstruction::Add, LinearInstruction::Return) => {
            writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    add rax, qword [rbp-{right_offset}]").unwrap();
            Some(3)
        }
        (LinearInstruction::Subtract, LinearInstruction::Return) => {
            writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    sub rax, qword [rbp-{right_offset}]").unwrap();
            Some(3)
        }
        (LinearInstruction::Multiply, LinearInstruction::Return) => {
            writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    imul rax, qword [rbp-{right_offset}]").unwrap();
            Some(3)
        }
        (LinearInstruction::Divide, LinearInstruction::Return) => {
            writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    cqo").unwrap();
            writeln!(output, "    idiv qword [rbp-{right_offset}]").unwrap();
            Some(3)
        }
        (compare, LinearInstruction::Return) => {
            let setcc = setcc_for_compare(compare)?;
            writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    cmp rax, qword [rbp-{right_offset}]").unwrap();
            writeln!(output, "    {setcc} al").unwrap();
            writeln!(output, "    movzx rax, al").unwrap();
            Some(3)
        }
        _ => None,
    }
}

fn try_render_local_compare_return(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    path: &[String],
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let left_offset = *local_offsets.get(&path[0])?;
    let right_value = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };
    let compare = instructions.get(instruction_index + 2)?;
    match instructions.get(instruction_index + 3) {
        Some(LinearInstruction::Return) => {
            let setcc = setcc_for_compare(compare)?;
            writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    cmp rax, {right_value}").unwrap();
            writeln!(output, "    {setcc} al").unwrap();
            writeln!(output, "    movzx rax, al").unwrap();
            Some(3)
        }
        _ => None,
    }
}

fn try_render_immediate_local_compare_return(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let left_value = match instructions.get(instruction_index) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };
    let path = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadReference(path)) if path.len() == 1 => path,
        _ => return None,
    };
    let right_offset = *local_offsets.get(&path[0])?;
    let compare = instructions.get(instruction_index + 2)?;

    match instructions.get(instruction_index + 3) {
        Some(LinearInstruction::Return) => {
            let setcc = setcc_for_compare(compare)?;
            writeln!(output, "    mov rax, {left_value}").unwrap();
            writeln!(output, "    cmp rax, qword [rbp-{right_offset}]").unwrap();
            writeln!(output, "    {setcc} al").unwrap();
            writeln!(output, "    movzx rax, al").unwrap();
            Some(3)
        }
        _ => None,
    }
}

fn try_render_integer_immediate_call(
    output: &mut String,
    user_functions: &[String],
    local_offsets: &BTreeMap<String, usize>,
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let (operands, cursor) =
        collect_simple_call_operands(local_offsets, instructions, instruction_index)?;

    if !matches!(operands.first(), Some(SimpleCallOperand::Integer(_))) {
        return None;
    }

    let callee = match instructions.get(cursor) {
        Some(LinearInstruction::Call {
            callee,
            argument_count,
        }) if *argument_count == operands.len()
            && callee.len() == 1
            && user_functions.contains(&callee[0]) =>
        {
            callee
        }
        _ => return None,
    };

    render_simple_user_call(output, callee, &operands)?;
    Some(cursor - instruction_index)
}

enum SimpleCallOperand {
    Integer(i64),
    LocalOffset(usize),
    ComputedLocal {
        offset: usize,
        immediate: i64,
        op: CallArithmeticOp,
    },
    ComputedLocalPair {
        left_offset: usize,
        right_offset: usize,
        op: CallArithmeticOp,
    },
}

#[derive(Copy, Clone)]
enum CallArithmeticOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

fn call_arithmetic_op(instruction: &LinearInstruction) -> Option<CallArithmeticOp> {
    match instruction {
        LinearInstruction::Add => Some(CallArithmeticOp::Add),
        LinearInstruction::Subtract => Some(CallArithmeticOp::Subtract),
        LinearInstruction::Multiply => Some(CallArithmeticOp::Multiply),
        LinearInstruction::Divide => Some(CallArithmeticOp::Divide),
        _ => None,
    }
}

fn render_simple_call_operand_into(output: &mut String, operand: &SimpleCallOperand, target: &str) {
    match operand {
        SimpleCallOperand::Integer(value) => {
            writeln!(output, "    mov {target}, {value}").unwrap();
        }
        SimpleCallOperand::LocalOffset(offset) => {
            writeln!(output, "    mov {target}, qword [rbp-{offset}]").unwrap();
        }
        SimpleCallOperand::ComputedLocal {
            offset,
            immediate,
            op,
        } => render_computed_call_operand(output, *offset, *immediate, *op, target),
        SimpleCallOperand::ComputedLocalPair {
            left_offset,
            right_offset,
            op,
        } => render_local_pair_call_operand(output, *left_offset, *right_offset, *op, target),
    }
}

fn render_local_pair_call_operand(
    output: &mut String,
    left_offset: usize,
    right_offset: usize,
    op: CallArithmeticOp,
    target: &str,
) {
    match op {
        CallArithmeticOp::Add => {
            writeln!(output, "    mov {target}, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    add {target}, qword [rbp-{right_offset}]").unwrap();
        }
        CallArithmeticOp::Subtract => {
            writeln!(output, "    mov {target}, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    sub {target}, qword [rbp-{right_offset}]").unwrap();
        }
        CallArithmeticOp::Multiply => {
            writeln!(output, "    mov {target}, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    imul {target}, qword [rbp-{right_offset}]").unwrap();
        }
        CallArithmeticOp::Divide => {
            writeln!(output, "    mov rax, qword [rbp-{left_offset}]").unwrap();
            writeln!(output, "    cqo").unwrap();
            writeln!(output, "    idiv qword [rbp-{right_offset}]").unwrap();
            writeln!(output, "    mov {target}, rax").unwrap();
        }
    }
}

fn render_computed_call_operand(
    output: &mut String,
    offset: usize,
    immediate: i64,
    op: CallArithmeticOp,
    target: &str,
) {
    match op {
        CallArithmeticOp::Add => {
            writeln!(output, "    mov {target}, qword [rbp-{offset}]").unwrap();
            writeln!(output, "    add {target}, {immediate}").unwrap();
        }
        CallArithmeticOp::Subtract => {
            writeln!(output, "    mov {target}, qword [rbp-{offset}]").unwrap();
            writeln!(output, "    sub {target}, {immediate}").unwrap();
        }
        CallArithmeticOp::Multiply => {
            writeln!(output, "    mov {target}, qword [rbp-{offset}]").unwrap();
            writeln!(output, "    imul {target}, {immediate}").unwrap();
        }
        CallArithmeticOp::Divide => {
            writeln!(output, "    mov rax, qword [rbp-{offset}]").unwrap();
            writeln!(output, "    cqo").unwrap();
            writeln!(output, "    mov rcx, {immediate}").unwrap();
            writeln!(output, "    idiv rcx").unwrap();
            writeln!(output, "    mov {target}, rax").unwrap();
        }
    }
}

fn push_simple_call_operand(output: &mut String, operand: &SimpleCallOperand) {
    match operand {
        SimpleCallOperand::Integer(value) => writeln!(output, "    push {value}").unwrap(),
        SimpleCallOperand::LocalOffset(offset) => {
            writeln!(output, "    push qword [rbp-{offset}]").unwrap()
        }
        SimpleCallOperand::ComputedLocal {
            offset,
            immediate,
            op,
        } => {
            render_computed_call_operand(output, *offset, *immediate, *op, "rax");
            writeln!(output, "    push rax").unwrap();
        }
        SimpleCallOperand::ComputedLocalPair {
            left_offset,
            right_offset,
            op,
        } => {
            render_local_pair_call_operand(output, *left_offset, *right_offset, *op, "rax");
            writeln!(output, "    push rax").unwrap();
        }
    }
}

fn render_simple_user_call(
    output: &mut String,
    callee: &[String],
    operands: &[SimpleCallOperand],
) -> Option<()> {
    let register_arg_count = operands.len().min(6);
    for (index, operand) in operands.iter().take(register_arg_count).enumerate() {
        let register = integer_argument_register(index)?;
        render_simple_call_operand_into(output, operand, register);
    }
    for operand in operands.iter().skip(register_arg_count).rev() {
        push_simple_call_operand(output, operand);
    }
    writeln!(output, "    call {}", native_symbol_name(&callee.join("."))).unwrap();
    let stack_arg_count = operands.len().saturating_sub(register_arg_count);
    if stack_arg_count > 0 {
        writeln!(output, "    add rsp, {}", stack_arg_count * 8).unwrap();
    }
    Some(())
}

fn collect_simple_call_operands(
    local_offsets: &BTreeMap<String, usize>,
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<(Vec<SimpleCallOperand>, usize)> {
    let mut operands = Vec::new();
    let mut cursor = instruction_index;

    loop {
        match instructions.get(cursor) {
            Some(LinearInstruction::LoadInteger(value)) => {
                operands.push(SimpleCallOperand::Integer(*value));
                cursor += 1;
            }
            Some(LinearInstruction::LoadReference(path)) if path.len() == 1 => {
                let offset = *local_offsets.get(&path[0])?;
                match (instructions.get(cursor + 1), instructions.get(cursor + 2)) {
                    (Some(LinearInstruction::LoadReference(other_path)), Some(op_instruction))
                        if other_path.len() == 1
                            && call_arithmetic_op(op_instruction).is_some() =>
                    {
                        let right_offset = *local_offsets.get(&other_path[0])?;
                        operands.push(SimpleCallOperand::ComputedLocalPair {
                            left_offset: offset,
                            right_offset,
                            op: call_arithmetic_op(op_instruction)
                                .expect("checked local pair operand op should exist"),
                        });
                        cursor += 3;
                    }
                    (Some(LinearInstruction::LoadInteger(immediate)), Some(op_instruction))
                        if call_arithmetic_op(op_instruction).is_some() =>
                    {
                        operands.push(SimpleCallOperand::ComputedLocal {
                            offset,
                            immediate: *immediate,
                            op: call_arithmetic_op(op_instruction)
                                .expect("checked computed operand op should exist"),
                        });
                        cursor += 3;
                    }
                    _ => {
                        operands.push(SimpleCallOperand::LocalOffset(offset));
                        cursor += 1;
                    }
                }
            }
            _ => break,
        }
    }

    Some((operands, cursor))
}

fn try_render_simple_user_call(
    output: &mut String,
    user_functions: &[String],
    local_offsets: &BTreeMap<String, usize>,
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let (operands, cursor) =
        collect_simple_call_operands(local_offsets, instructions, instruction_index)?;

    let callee = match instructions.get(cursor) {
        Some(LinearInstruction::Call {
            callee,
            argument_count,
        }) if *argument_count == operands.len()
            && callee.len() == 1
            && user_functions.contains(&callee[0]) =>
        {
            callee
        }
        _ => return None,
    };

    render_simple_user_call(output, callee, &operands)?;
    Some(cursor - instruction_index)
}

fn try_render_single_local_call(
    output: &mut String,
    user_functions: &[String],
    local_offsets: &BTreeMap<String, usize>,
    path: &[String],
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let offset = *local_offsets.get(&path[0])?;
    let callee = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::Call {
            callee,
            argument_count: 1,
        }) if callee.len() == 1 && user_functions.contains(&callee[0]) => callee,
        _ => return None,
    };

    writeln!(output, "    mov rdi, qword [rbp-{offset}]").unwrap();
    writeln!(output, "    call {}", native_symbol_name(&callee.join("."))).unwrap();
    Some(1)
}

fn try_render_local_compute_call(
    output: &mut String,
    user_functions: &[String],
    local_offsets: &BTreeMap<String, usize>,
    _path: &[String],
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    try_render_simple_user_call(
        output,
        user_functions,
        local_offsets,
        instructions,
        instruction_index,
    )
}

fn arithmetic_mnemonic(instruction: &LinearInstruction) -> Option<&'static str> {
    match instruction {
        LinearInstruction::Add => Some("add"),
        LinearInstruction::Subtract => Some("sub"),
        LinearInstruction::Multiply => Some("imul"),
        _ => None,
    }
}

fn try_render_integer_local_initializer(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let left = match instructions.get(instruction_index) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };
    let right = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };

    match instructions.get(instruction_index + 2)? {
        instruction if arithmetic_mnemonic(instruction).is_some() => {
            let mnemonic = arithmetic_mnemonic(instruction).expect("mnemonic should exist");
            match instructions.get(instruction_index + 3) {
                Some(LinearInstruction::StoreLocal(name)) => {
                    let offset = *local_offsets.get(name)?;
                    writeln!(output, "    mov rax, {left}").unwrap();
                    writeln!(output, "    {mnemonic} rax, {right}").unwrap();
                    writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
                    Some(3)
                }
                _ => None,
            }
        }
        LinearInstruction::Divide => match instructions.get(instruction_index + 3) {
            Some(LinearInstruction::StoreLocal(name)) => {
                let offset = *local_offsets.get(name)?;
                writeln!(output, "    mov rax, {left}").unwrap();
                writeln!(output, "    cqo").unwrap();
                writeln!(output, "    mov rcx, {right}").unwrap();
                writeln!(output, "    idiv rcx").unwrap();
                writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
                Some(3)
            }
            _ => None,
        },
        _ => None,
    }
}

fn commutative_arithmetic_mnemonic(instruction: &LinearInstruction) -> Option<&'static str> {
    match instruction {
        LinearInstruction::Add => Some("add"),
        LinearInstruction::Multiply => Some("imul"),
        _ => None,
    }
}

fn try_render_commuted_local_integer_update(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let value = match instructions.get(instruction_index) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };
    let path = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadReference(path)) if path.len() == 1 => path,
        _ => return None,
    };
    let mnemonic = commutative_arithmetic_mnemonic(instructions.get(instruction_index + 2)?)?;

    match instructions.get(instruction_index + 3) {
        Some(LinearInstruction::StoreReference(target))
            if target.len() == 1 && target[0] == path[0] =>
        {
            let offset = *local_offsets.get(&path[0])?;
            writeln!(output, "    mov rax, qword [rbp-{offset}]").unwrap();
            writeln!(output, "    {mnemonic} rax, {value}").unwrap();
            writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
            Some(3)
        }
        _ => None,
    }
}

fn try_render_immediate_local_integer_update(
    output: &mut String,
    local_offsets: &BTreeMap<String, usize>,
    instructions: &[LinearInstruction],
    instruction_index: usize,
) -> Option<usize> {
    let value = match instructions.get(instruction_index) {
        Some(LinearInstruction::LoadInteger(value)) => *value,
        _ => return None,
    };
    let path = match instructions.get(instruction_index + 1) {
        Some(LinearInstruction::LoadReference(path)) if path.len() == 1 => path,
        _ => return None,
    };

    match instructions.get(instruction_index + 2)? {
        LinearInstruction::Subtract => match instructions.get(instruction_index + 3) {
            Some(LinearInstruction::StoreReference(target))
                if target.len() == 1 && target[0] == path[0] =>
            {
                let offset = *local_offsets.get(&path[0])?;
                writeln!(output, "    mov rax, {value}").unwrap();
                writeln!(output, "    sub rax, qword [rbp-{offset}]").unwrap();
                writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
                Some(3)
            }
            _ => None,
        },
        LinearInstruction::Divide => match instructions.get(instruction_index + 3) {
            Some(LinearInstruction::StoreReference(target))
                if target.len() == 1 && target[0] == path[0] =>
            {
                let offset = *local_offsets.get(&path[0])?;
                writeln!(output, "    mov rax, {value}").unwrap();
                writeln!(output, "    cqo").unwrap();
                writeln!(output, "    idiv qword [rbp-{offset}]").unwrap();
                writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
                Some(3)
            }
            _ => None,
        },
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

    match instructions.get(instruction_index + 2)? {
        instruction if arithmetic_mnemonic(instruction).is_some() => {
            let mnemonic = arithmetic_mnemonic(instruction).expect("mnemonic should exist");
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
        LinearInstruction::Divide => match instructions.get(instruction_index + 3) {
            Some(LinearInstruction::StoreReference(target))
                if target.len() == 1 && target[0] == path[0] =>
            {
                writeln!(output, "    mov rax, qword [rbp-{offset}]").unwrap();
                writeln!(output, "    cqo").unwrap();
                writeln!(output, "    mov rcx, {value}").unwrap();
                writeln!(output, "    idiv rcx").unwrap();
                writeln!(output, "    mov qword [rbp-{offset}], rax").unwrap();
                Some(3)
            }
            _ => None,
        },
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
    fn lowers_single_local_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes value as integer returns integer
    return value

define function main returns integer
    mutable n as integer be 7
    return helper(n)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    mov qword [rbp-8], 7"));
        assert!(native.contains("    mov rdi, qword [rbp-8]"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
    }

    #[test]
    fn lowers_computed_single_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes value as integer returns integer
    return value

define function main returns integer
    mutable n as integer be 5
    return helper(n + 1)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov rdi, qword [rbp-8]"));
        assert!(native.contains("    add rdi, 1"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("add_pop"));
    }

    #[test]
    fn lowers_local_sum_single_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes value as integer returns integer
    return value

define function main returns integer
    mutable n as integer be 5
    mutable m as integer be 3
    return helper(n + m)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov qword [rbp-16], 3"));
        assert!(native.contains("    mov rdi, qword [rbp-8]"));
        assert!(native.contains("    add rdi, qword [rbp-16]"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("load m"));
        assert!(!native.contains("add_pop"));
    }

    #[test]
    fn lowers_multiplied_single_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes value as integer returns integer
    return value

define function main returns integer
    mutable n as integer be 5
    return helper(n * 2)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov rdi, qword [rbp-8]"));
        assert!(native.contains("    imul rdi, 2"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("mul_pop"));
    }

    #[test]
    fn lowers_divided_single_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes value as integer returns integer
    return value

define function main returns integer
    mutable n as integer be 8
    return helper(n / 2)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    mov qword [rbp-8], 8"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cqo"));
        assert!(native.contains("    mov rcx, 2"));
        assert!(native.contains("    idiv rcx"));
        assert!(native.contains("    mov rdi, rax"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("div_pop"));
    }

    #[test]
    fn lowers_subtracted_single_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes value as integer returns integer
    return value

define function main returns integer
    mutable n as integer be 5
    return helper(n - 2)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov rdi, qword [rbp-8]"));
        assert!(native.contains("    sub rdi, 2"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("sub_pop"));
    }

    #[test]
    fn lowers_single_integer_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes value as integer returns integer
    return value

define function main returns integer
    return helper(7)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 7"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("push_int 7"));
        assert!(!native.contains("load value"));
    }

    #[test]
    fn lowers_mixed_immediate_and_local_two_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    return left + right

define function main returns integer
    mutable n as integer be 5
    return helper(7, n)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov rdi, 7"));
        assert!(native.contains("    mov rsi, qword [rbp-8]"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
    }

    #[test]
    fn lowers_computed_and_local_two_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    return left + right

define function main returns integer
    mutable n as integer be 5
    mutable m as integer be 3
    return helper(n + 1, m)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov qword [rbp-16], 3"));
        assert!(native.contains("    mov rdi, qword [rbp-8]"));
        assert!(native.contains("    add rdi, 1"));
        assert!(native.contains("    mov rsi, qword [rbp-16]"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("load m"));
        assert!(!native.contains("add_pop"));
    }

    #[test]
    fn lowers_two_computed_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    return left + right

define function main returns integer
    mutable n as integer be 5
    mutable m as integer be 3
    return helper(n + 1, m * 2)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov qword [rbp-16], 3"));
        assert!(native.contains("    mov rdi, qword [rbp-8]"));
        assert!(native.contains("    add rdi, 1"));
        assert!(native.contains("    mov rsi, qword [rbp-16]"));
        assert!(native.contains("    imul rsi, 2"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("load m"));
        assert!(!native.contains("add_pop"));
        assert!(!native.contains("mul_pop"));
    }

    #[test]
    fn lowers_two_integer_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    return left + right

define function main returns integer
    return helper(7, 5)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 7"));
        assert!(native.contains("    mov rsi, 5"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("push_int 7"));
        assert!(!native.contains("push_int 5"));
        assert!(!native.contains("load left"));
        assert!(!native.contains("load right"));
    }

    #[test]
    fn lowers_two_integer_argument_user_calls_with_subtract_body() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    return left - right

define function main returns integer
    return helper(7, 5)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    sub rax, qword [rbp-16]"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 7"));
        assert!(native.contains("    mov rsi, 5"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load left"));
        assert!(!native.contains("load right"));
        assert!(!native.contains("sub_pop"));
    }

    #[test]
    fn lowers_two_integer_argument_user_calls_with_multiply_body() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    return left * right

define function main returns integer
    return helper(7, 5)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    imul rax, qword [rbp-16]"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 7"));
        assert!(native.contains("    mov rsi, 5"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load left"));
        assert!(!native.contains("load right"));
        assert!(!native.contains("mul_pop"));
    }

    #[test]
    fn lowers_two_integer_argument_user_calls_with_divide_body() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    return left / right

define function main returns integer
    return helper(8, 2)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cqo"));
        assert!(native.contains("    idiv qword [rbp-16]"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 8"));
        assert!(native.contains("    mov rsi, 2"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("load left"));
        assert!(!native.contains("load right"));
        assert!(!native.contains("div_pop"));
    }

    #[test]
    fn lowers_single_integer_argument_user_calls_with_less_than_immediate_boolean_return() {
        let source = r#"module demo.native

define function helper takes value as integer returns boolean
    return value < 3

define function main returns integer
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, 3"));
        assert!(native.contains("    setl al"));
        assert!(native.contains("    movzx rax, al"));
        assert!(!native.contains("cmp_lt_pop"));
    }

    #[test]
    fn lowers_immediate_less_than_local_boolean_return() {
        let source = r#"module demo.native

define function helper takes value as integer returns boolean
    return 3 < value

define function main returns integer
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov rax, 3"));
        assert!(native.contains("    cmp rax, qword [rbp-8]"));
        assert!(native.contains("    setl al"));
        assert!(native.contains("    movzx rax, al"));
        assert!(!native.contains("cmp_lt_pop"));
    }

    #[test]
    fn lowers_single_integer_argument_user_calls_with_equal_immediate_boolean_return() {
        let source = r#"module demo.native

define function helper takes value as integer returns boolean
    return value == 3

define function main returns integer
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, 3"));
        assert!(native.contains("    sete al"));
        assert!(native.contains("    movzx rax, al"));
        assert!(!native.contains("cmp_eq_pop"));
    }

    #[test]
    fn lowers_two_integer_argument_user_calls_with_equal_boolean_return() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns boolean
    return left == right

define function main returns integer
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, qword [rbp-16]"));
        assert!(native.contains("    sete al"));
        assert!(native.contains("    movzx rax, al"));
        assert!(!native.contains("cmp_eq_pop"));
    }

    #[test]
    fn lowers_two_integer_argument_user_calls_with_less_than_boolean_return() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns boolean
    return left < right

define function main returns integer
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, qword [rbp-16]"));
        assert!(native.contains("    setl al"));
        assert!(native.contains("    movzx rax, al"));
        assert!(!native.contains("cmp_lt_pop"));
    }

    #[test]
    fn lowers_two_integer_argument_user_calls_with_less_than_branch_body() {
        let source = r#"module demo.native

define function helper takes left as integer, right as integer returns integer
    if left < right
        return 1
    else
        return 0

define function main returns integer
    return helper(2, 5)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 16"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cmp rax, qword [rbp-16]"));
        assert!(native.contains("    jge helper_if_else_0"));
        assert!(!native.contains("cmp_lt_pop"));
        assert!(!native.contains("jmp_if_false_pop helper_if_else_0"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 2"));
        assert!(native.contains("    mov rsi, 5"));
        assert!(native.contains("    call helper"));
    }

    #[test]
    fn lowers_three_integer_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes first as integer, second as integer, third as integer returns integer
    return third

define function main returns integer
    return helper(7, 5, 3)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 24"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov qword [rbp-24], rdx"));
        assert!(native.contains("    mov rax, qword [rbp-24]"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 7"));
        assert!(native.contains("    mov rsi, 5"));
        assert!(native.contains("    mov rdx, 3"));
        assert!(native.contains("    call helper"));
        assert!(!native.contains("push_int 7"));
        assert!(!native.contains("push_int 5"));
        assert!(!native.contains("push_int 3"));
        assert!(!native.contains("load third"));
    }

    #[test]
    fn lowers_seven_integer_argument_user_calls() {
        let source = r#"module demo.native

define function helper takes a as integer, b as integer, c as integer, d as integer, e as integer, f as integer, g as integer returns integer
    return g

define function main returns integer
    return helper(1, 2, 3, 4, 5, 6, 7)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("helper:"));
        assert!(native.contains("    sub rsp, 56"));
        assert!(native.contains("    mov qword [rbp-8], rdi"));
        assert!(native.contains("    mov qword [rbp-16], rsi"));
        assert!(native.contains("    mov qword [rbp-24], rdx"));
        assert!(native.contains("    mov qword [rbp-32], rcx"));
        assert!(native.contains("    mov qword [rbp-40], r8"));
        assert!(native.contains("    mov qword [rbp-48], r9"));
        assert!(native.contains("    mov rax, qword [rbp-56]"));
        assert!(native.contains("main:"));
        assert!(native.contains("    mov rdi, 1"));
        assert!(native.contains("    mov rsi, 2"));
        assert!(native.contains("    mov rdx, 3"));
        assert!(native.contains("    mov rcx, 4"));
        assert!(native.contains("    mov r8, 5"));
        assert!(native.contains("    mov r9, 6"));
        assert!(native.contains("    push 7"));
        assert!(native.contains("    call helper"));
        assert!(native.contains("    add rsp, 8"));
        assert!(!native.contains("push_int 7"));
    }

    #[test]
    fn lowers_stack_passed_computed_user_call_arguments() {
        let source = r#"module demo.native

define function helper takes a as integer, b as integer, c as integer, d as integer, e as integer, f as integer, g as integer returns integer
    return g

define function main returns integer
    mutable n as integer be 5
    return helper(1, 2, 3, 4, 5, 6, n + 2)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("main:"));
        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 5"));
        assert!(native.contains("    mov rdi, 1"));
        assert!(native.contains("    mov rsi, 2"));
        assert!(native.contains("    mov rdx, 3"));
        assert!(native.contains("    mov rcx, 4"));
        assert!(native.contains("    mov r8, 5"));
        assert!(native.contains("    mov r9, 6"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    add rax, 2"));
        assert!(native.contains("    push rax"));
        assert!(native.contains("    call helper"));
        assert!(native.contains("    add rsp, 8"));
        assert!(!native.contains("load n"));
        assert!(!native.contains("add_pop"));
        assert!(!native.contains("push_int 1"));
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
    fn lowers_integer_local_add_initializer_into_stack_slot() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 1 + 2
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov rax, 1"));
        assert!(native.contains("    add rax, 2"));
        assert!(native.contains("    mov qword [rbp-8], rax"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(!native.contains("store_local_pop score"));
        assert!(!native.contains("add_pop"));
    }

    #[test]
    fn lowers_commuted_integer_local_add_updates_into_stack_slots() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 1
    set score to 2 + score
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
        assert!(!native.contains("store_pop score"));
        assert!(!native.contains("add_pop"));
    }

    #[test]
    fn lowers_integer_minus_local_updates_into_stack_slots() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 3
    set score to 10 - score
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 3"));
        assert!(native.contains("    mov rax, 10"));
        assert!(native.contains("    sub rax, qword [rbp-8]"));
        assert!(native.contains("    mov qword [rbp-8], rax"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(!native.contains("store_pop score"));
        assert!(!native.contains("sub_pop"));
    }

    #[test]
    fn lowers_integer_local_multiply_initializer_into_stack_slot() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 3 * 4
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov rax, 3"));
        assert!(native.contains("    imul rax, 4"));
        assert!(native.contains("    mov qword [rbp-8], rax"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(!native.contains("store_local_pop score"));
        assert!(!native.contains("mul_pop"));
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
    fn lowers_integer_divided_by_local_updates_into_stack_slots() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 2
    set score to 10 / score
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 2"));
        assert!(native.contains("    mov rax, 10"));
        assert!(native.contains("    cqo"));
        assert!(native.contains("    idiv qword [rbp-8]"));
        assert!(native.contains("    mov qword [rbp-8], rax"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(!native.contains("store_pop score"));
        assert!(!native.contains("div_pop"));
    }

    #[test]
    fn lowers_integer_local_division_updates_into_stack_slots() {
        let source = r#"module demo.native

define function main returns integer
    mutable score as integer be 8
    set score to score / 2
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("    sub rsp, 8"));
        assert!(native.contains("    mov qword [rbp-8], 8"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(native.contains("    cqo"));
        assert!(native.contains("    mov rcx, 2"));
        assert!(native.contains("    idiv rcx"));
        assert!(native.contains("    mov qword [rbp-8], rax"));
        assert!(native.contains("    mov rax, qword [rbp-8]"));
        assert!(!native.contains("store_pop score"));
        assert!(!native.contains("div_pop"));
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
