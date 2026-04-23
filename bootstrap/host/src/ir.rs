use crate::ast::{Declaration, FieldDeclaration, Parameter, SourceFile, Statement};
use std::fmt::{self, Write};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub module_path: Vec<String>,
    pub imports: Vec<Vec<String>>,
    pub declarations: Vec<IrDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrDeclaration {
    Record(IrRecord),
    Function(IrFunction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrRecord {
    pub name: String,
    pub fields: Vec<IrField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrField {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunction {
    pub name: String,
    pub parameters: Vec<IrParameter>,
    pub return_type: String,
    pub body: Vec<IrStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrParameter {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrCompareOp {
    Equal,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrExpr {
    Integer(i64),
    Boolean(bool),
    Text(String),
    Reference(Vec<String>),
    Binary {
        left: Box<IrExpr>,
        op: IrBinaryOp,
        right: Box<IrExpr>,
    },
    Compare {
        left: Box<IrExpr>,
        op: IrCompareOp,
        right: Box<IrExpr>,
    },
    Call {
        callee: Vec<String>,
        arguments: Vec<IrExpr>,
    },
    Opaque(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrStatement {
    Let {
        name: String,
        value: IrExpr,
    },
    Mutable {
        name: String,
        type_name: String,
        value: IrExpr,
    },
    Set {
        target: Vec<String>,
        value: IrExpr,
    },
    If {
        condition: IrExpr,
        then_body: Vec<IrStatement>,
        else_body: Vec<IrStatement>,
    },
    RepeatWhile {
        condition: IrExpr,
        body: Vec<IrStatement>,
    },
    Return {
        value: Option<IrExpr>,
    },
    Expr {
        value: IrExpr,
    },
}

pub fn lower_source_file(file: &SourceFile) -> IrProgram {
    IrProgram {
        module_path: file.module_path.clone(),
        imports: file.imports.clone(),
        declarations: file.declarations.iter().map(lower_declaration).collect(),
    }
}

pub fn render_program(program: &IrProgram) -> String {
    let mut output = String::new();
    writeln!(&mut output, "module {}", program.module_path.join(".")).unwrap();

    for import in &program.imports {
        writeln!(&mut output, "import {}", import.join(".")).unwrap();
    }

    if !program.imports.is_empty() && !program.declarations.is_empty() {
        output.push('\n');
    }

    for (index, declaration) in program.declarations.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        render_declaration(&mut output, declaration, 0).unwrap();
    }

    output
}

fn lower_declaration(declaration: &Declaration) -> IrDeclaration {
    match declaration {
        Declaration::Record(record) => IrDeclaration::Record(IrRecord {
            name: record.name.clone(),
            fields: record.fields.iter().map(lower_field).collect(),
        }),
        Declaration::Function(function) => IrDeclaration::Function(IrFunction {
            name: function.name.clone(),
            parameters: function.parameters.iter().map(lower_parameter).collect(),
            return_type: function.return_type.name.clone(),
            body: lower_statements(&function.body),
        }),
    }
}

fn lower_field(field: &FieldDeclaration) -> IrField {
    IrField {
        name: field.name.clone(),
        type_name: field.field_type.name.clone(),
    }
}

fn lower_parameter(parameter: &Parameter) -> IrParameter {
    IrParameter {
        name: parameter.name.clone(),
        type_name: parameter.parameter_type.name.clone(),
    }
}

fn lower_statements(statements: &[Statement]) -> Vec<IrStatement> {
    statements.iter().map(lower_statement).collect()
}

fn lower_reference_path(reference: &str) -> Vec<String> {
    reference
        .split('.')
        .map(|segment| segment.to_string())
        .collect()
}

fn lower_expression(expression: &str) -> IrExpr {
    let expression = expression.trim();

    if let Some(compare) = lower_compare_expression(expression) {
        return compare;
    }

    if let Some(binary) = lower_binary_expression(expression) {
        return binary;
    }

    if let Some(call) = lower_call_expression(expression) {
        return call;
    }

    if let Ok(value) = expression.parse::<i64>() {
        return IrExpr::Integer(value);
    }

    if expression == "true" {
        return IrExpr::Boolean(true);
    }

    if expression == "false" {
        return IrExpr::Boolean(false);
    }

    if expression.starts_with('"') && expression.ends_with('"') && expression.len() >= 2 {
        return IrExpr::Text(expression[1..expression.len() - 1].to_string());
    }

    if is_parenthesized(expression) {
        return lower_expression(&expression[1..expression.len() - 1]);
    }

    if is_reference_path(expression) {
        return IrExpr::Reference(lower_reference_path(expression));
    }

    IrExpr::Opaque(expression.to_string())
}

fn lower_binary_expression(expression: &str) -> Option<IrExpr> {
    for operators in [&["+", "-"][..], &["*", "/"][..]] {
        if let Some((left, operator, right)) = split_top_level_binary(expression, operators) {
            let op = match operator.as_str() {
                "+" => IrBinaryOp::Add,
                "-" => IrBinaryOp::Subtract,
                "*" => IrBinaryOp::Multiply,
                "/" => IrBinaryOp::Divide,
                _ => unreachable!("unexpected operator"),
            };
            return Some(IrExpr::Binary {
                left: Box::new(lower_expression(&left)),
                op,
                right: Box::new(lower_expression(&right)),
            });
        }
    }
    None
}

fn lower_compare_expression(expression: &str) -> Option<IrExpr> {
    let operators = ["==", "<=", ">=", "<", ">"];
    let (left, operator, right) = split_top_level_operator(expression, &operators)?;
    let op = match operator.as_str() {
        "==" => IrCompareOp::Equal,
        "<=" => IrCompareOp::LessEqual,
        ">=" => IrCompareOp::GreaterEqual,
        "<" => IrCompareOp::Less,
        ">" => IrCompareOp::Greater,
        _ => unreachable!("unexpected comparison operator"),
    };

    Some(IrExpr::Compare {
        left: Box::new(lower_expression(&left)),
        op,
        right: Box::new(lower_expression(&right)),
    })
}

fn split_top_level_binary(
    expression: &str,
    operators: &[&str],
) -> Option<(String, String, String)> {
    split_top_level_operator(expression, operators)
}

fn split_top_level_operator(
    expression: &str,
    operators: &[&str],
) -> Option<(String, String, String)> {
    let chars: Vec<char> = expression.chars().collect();
    let mut depth = 0;
    let mut in_text = false;

    for index in (0..chars.len()).rev() {
        let ch = chars[index];
        match ch {
            '"' => in_text = !in_text,
            ')' if !in_text => depth += 1,
            '(' if !in_text => depth -= 1,
            _ => {}
        }

        if in_text || depth != 0 {
            continue;
        }

        for operator in operators {
            let op_len = operator.len();
            if index + op_len > expression.len() {
                continue;
            }
            if &expression[index..index + op_len] != *operator {
                continue;
            }
            if index == 0 {
                continue;
            }
            let left = expression[..index].trim();
            let right = expression[index + op_len..].trim();
            if left.is_empty() || right.is_empty() {
                continue;
            }
            return Some((left.to_string(), operator.to_string(), right.to_string()));
        }
    }

    None
}

fn is_parenthesized(expression: &str) -> bool {
    if !expression.starts_with('(') || !expression.ends_with(')') {
        return false;
    }

    let mut depth = 0;
    let mut in_text = false;
    for (index, ch) in expression.char_indices() {
        match ch {
            '"' => in_text = !in_text,
            '(' if !in_text => depth += 1,
            ')' if !in_text => {
                depth -= 1;
                if depth == 0 && index != expression.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }

    depth == 0 && !in_text
}

fn lower_call_expression(expression: &str) -> Option<IrExpr> {
    let open_paren = expression.find('(')?;
    if !expression.ends_with(')') {
        return None;
    }

    let callee = expression[..open_paren].trim();
    if !is_reference_path(callee) {
        return None;
    }

    let arguments_source = &expression[open_paren + 1..expression.len() - 1];
    let arguments = split_top_level_arguments(arguments_source)
        .into_iter()
        .map(|argument| lower_expression(&argument))
        .collect();

    Some(IrExpr::Call {
        callee: lower_reference_path(callee),
        arguments,
    })
}

fn split_top_level_arguments(arguments: &str) -> Vec<String> {
    if arguments.trim().is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut in_text = false;
    let chars: Vec<char> = arguments.chars().collect();

    for (index, ch) in chars.iter().enumerate() {
        match ch {
            '"' => in_text = !in_text,
            '(' if !in_text => depth += 1,
            ')' if !in_text => depth -= 1,
            ',' if !in_text && depth == 0 => {
                result.push(arguments[start..index].trim().to_string());
                start = index + 1;
            }
            _ => {}
        }
    }

    result.push(arguments[start..].trim().to_string());
    result
}

fn is_reference_path(expression: &str) -> bool {
    !expression.is_empty() && expression.split('.').all(|segment| is_identifier(segment))
}

fn is_identifier(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn lower_statement(statement: &Statement) -> IrStatement {
    match statement {
        Statement::Let { name, value } => IrStatement::Let {
            name: name.clone(),
            value: lower_expression(value),
        },
        Statement::Mutable {
            name,
            value_type,
            value,
        } => IrStatement::Mutable {
            name: name.clone(),
            type_name: value_type.name.clone(),
            value: lower_expression(value),
        },
        Statement::Set { target, value } => IrStatement::Set {
            target: lower_reference_path(target),
            value: lower_expression(value),
        },
        Statement::If {
            condition,
            then_body,
            else_body,
        } => IrStatement::If {
            condition: lower_expression(condition),
            then_body: lower_statements(then_body),
            else_body: lower_statements(else_body),
        },
        Statement::RepeatWhile { condition, body } => IrStatement::RepeatWhile {
            condition: lower_expression(condition),
            body: lower_statements(body),
        },
        Statement::Return { value } => IrStatement::Return {
            value: value.as_deref().map(lower_expression),
        },
        Statement::Expression { value } => IrStatement::Expr {
            value: lower_expression(value),
        },
    }
}

fn render_declaration(
    output: &mut String,
    declaration: &IrDeclaration,
    indent: usize,
) -> fmt::Result {
    match declaration {
        IrDeclaration::Record(record) => {
            writeln!(output, "{}record {}", indent_text(indent), record.name)?;
            for field in &record.fields {
                writeln!(
                    output,
                    "{}field {}: {}",
                    indent_text(indent + 1),
                    field.name,
                    field.type_name
                )?;
            }
        }
        IrDeclaration::Function(function) => {
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| format!("{}: {}", parameter.name, parameter.type_name))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                output,
                "{}fn {}({}) -> {}",
                indent_text(indent),
                function.name,
                parameters,
                function.return_type
            )?;
            for statement in &function.body {
                render_statement(output, statement, indent + 1)?;
            }
        }
    }
    Ok(())
}

fn render_statement(output: &mut String, statement: &IrStatement, indent: usize) -> fmt::Result {
    match statement {
        IrStatement::Let { name, value } => {
            writeln!(
                output,
                "{}let {} = {}",
                indent_text(indent),
                name,
                render_expression(value)
            )?;
        }
        IrStatement::Mutable {
            name,
            type_name,
            value,
        } => {
            writeln!(
                output,
                "{}mutable {}: {} = {}",
                indent_text(indent),
                name,
                type_name,
                render_expression(value)
            )?;
        }
        IrStatement::Set { target, value } => {
            writeln!(
                output,
                "{}set {} = {}",
                indent_text(indent),
                target.join("."),
                render_expression(value)
            )?;
        }
        IrStatement::If {
            condition,
            then_body,
            else_body,
        } => {
            writeln!(
                output,
                "{}if {}",
                indent_text(indent),
                render_expression(condition)
            )?;
            for statement in then_body {
                render_statement(output, statement, indent + 1)?;
            }
            if !else_body.is_empty() {
                writeln!(output, "{}else", indent_text(indent))?;
                for statement in else_body {
                    render_statement(output, statement, indent + 1)?;
                }
            }
        }
        IrStatement::RepeatWhile { condition, body } => {
            writeln!(
                output,
                "{}repeat_while {}",
                indent_text(indent),
                render_expression(condition)
            )?;
            for statement in body {
                render_statement(output, statement, indent + 1)?;
            }
        }
        IrStatement::Return { value } => match value {
            Some(value) => writeln!(
                output,
                "{}return {}",
                indent_text(indent),
                render_expression(value)
            )?,
            None => writeln!(output, "{}return", indent_text(indent))?,
        },
        IrStatement::Expr { value } => {
            writeln!(
                output,
                "{}expr {}",
                indent_text(indent),
                render_expression(value)
            )?;
        }
    }
    Ok(())
}

fn render_expression(expression: &IrExpr) -> String {
    match expression {
        IrExpr::Integer(value) => value.to_string(),
        IrExpr::Boolean(value) => value.to_string(),
        IrExpr::Text(value) => format!("\"{value}\""),
        IrExpr::Reference(path) => path.join("."),
        IrExpr::Binary { left, op, right } => format!(
            "{} {} {}",
            render_expression_with_precedence(left, binary_precedence(op)),
            render_binary_op(op),
            render_expression_with_precedence(right, binary_precedence(op) + 1)
        ),
        IrExpr::Compare { left, op, right } => format!(
            "{} {} {}",
            render_expression_with_precedence(left, compare_precedence()),
            render_compare_op(op),
            render_expression_with_precedence(right, compare_precedence() + 1)
        ),
        IrExpr::Call { callee, arguments } => format!(
            "{}({})",
            callee.join("."),
            arguments
                .iter()
                .map(render_expression)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        IrExpr::Opaque(source) => source.clone(),
    }
}

fn render_expression_with_precedence(expression: &IrExpr, parent_precedence: u8) -> String {
    match expression {
        IrExpr::Binary { op, .. } if binary_precedence(op) < parent_precedence => {
            format!("({})", render_expression(expression))
        }
        IrExpr::Compare { .. } if compare_precedence() < parent_precedence => {
            format!("({})", render_expression(expression))
        }
        _ => render_expression(expression),
    }
}

fn render_binary_op(op: &IrBinaryOp) -> &'static str {
    match op {
        IrBinaryOp::Add => "+",
        IrBinaryOp::Subtract => "-",
        IrBinaryOp::Multiply => "*",
        IrBinaryOp::Divide => "/",
    }
}

fn render_compare_op(op: &IrCompareOp) -> &'static str {
    match op {
        IrCompareOp::Equal => "==",
        IrCompareOp::Less => "<",
        IrCompareOp::Greater => ">",
        IrCompareOp::LessEqual => "<=",
        IrCompareOp::GreaterEqual => ">=",
    }
}

fn binary_precedence(op: &IrBinaryOp) -> u8 {
    match op {
        IrBinaryOp::Add | IrBinaryOp::Subtract => 1,
        IrBinaryOp::Multiply | IrBinaryOp::Divide => 2,
    }
}

fn compare_precedence() -> u8 {
    0
}

fn indent_text(indent: usize) -> String {
    "    ".repeat(indent)
}

#[cfg(test)]
mod tests {
    use super::{lower_source_file, render_program, IrBinaryOp, IrCompareOp, IrExpr};
    use crate::parser::parse_source;
    use crate::typecheck::validate_source_file;

    #[test]
    fn lowers_and_renders_nested_control_flow() {
        let source = r#"module demo.ir

import standard.io

define function main takes ready as boolean returns integer
    if ready
        io.print_line("go")
        return 1
    else
        repeat while false
            io.print_line("wait")

    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let program = lower_source_file(&file);
        let rendered = render_program(&program);

        assert!(rendered.contains("module demo.ir"));
        assert!(rendered.contains("fn main(ready: boolean) -> integer"));
        assert!(rendered.contains("if ready"));
        assert!(rendered.contains("expr io.print_line(\"go\")"));
        assert!(rendered.contains("repeat_while false"));
        assert!(rendered.contains("return 0"));

        let function = match &program.declarations[0] {
            super::IrDeclaration::Function(function) => function,
            other => panic!("expected function declaration, got {other:?}"),
        };

        match &function.body[0] {
            super::IrStatement::If { then_body, .. } => match &then_body[0] {
                super::IrStatement::Expr { value } => {
                    assert_eq!(
                        value,
                        &IrExpr::Call {
                            callee: vec!["io".to_string(), "print_line".to_string()],
                            arguments: vec![IrExpr::Text("go".to_string())],
                        }
                    );
                }
                other => panic!("expected expression statement, got {other:?}"),
            },
            other => panic!("expected if statement, got {other:?}"),
        }
    }

    #[test]
    fn lowers_set_targets_as_reference_paths() {
        let source = r#"module demo.ir_paths

define record counter
    value as integer

define function main returns integer
    mutable state as counter be counter(value 1)
    set state.value to 2
    return state.value
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let program = lower_source_file(&file);

        let function = match &program.declarations[1] {
            super::IrDeclaration::Function(function) => function,
            other => panic!("expected function declaration, got {other:?}"),
        };

        match &function.body[1] {
            super::IrStatement::Set { target, value } => {
                assert_eq!(target, &vec!["state".to_string(), "value".to_string()]);
                assert_eq!(value, &IrExpr::Integer(2));
            }
            other => panic!("expected set statement, got {other:?}"),
        }

        match &function.body[2] {
            super::IrStatement::Return { value } => {
                assert_eq!(
                    value,
                    &Some(IrExpr::Reference(vec![
                        "state".to_string(),
                        "value".to_string()
                    ]))
                );
            }
            other => panic!("expected return statement, got {other:?}"),
        }
    }

    #[test]
    fn lowers_binary_expressions_with_precedence() {
        let source = r#"module demo.ir_binary

define function main takes score as integer, bonus as integer returns integer
    return score + bonus * 2
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let program = lower_source_file(&file);

        let function = match &program.declarations[0] {
            super::IrDeclaration::Function(function) => function,
            other => panic!("expected function declaration, got {other:?}"),
        };

        match &function.body[0] {
            super::IrStatement::Return { value } => {
                assert_eq!(
                    value,
                    &Some(IrExpr::Binary {
                        left: Box::new(IrExpr::Reference(vec!["score".to_string()])),
                        op: IrBinaryOp::Add,
                        right: Box::new(IrExpr::Binary {
                            left: Box::new(IrExpr::Reference(vec!["bonus".to_string()])),
                            op: IrBinaryOp::Multiply,
                            right: Box::new(IrExpr::Integer(2)),
                        }),
                    })
                );
            }
            other => panic!("expected return statement, got {other:?}"),
        }
    }

    #[test]
    fn lowers_comparison_expressions_after_arithmetic() {
        let source = r#"module demo.ir_compare

define function main takes score as integer, bonus as integer, limit as integer returns boolean
    return score + bonus * 2 <= limit
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let program = lower_source_file(&file);

        let function = match &program.declarations[0] {
            super::IrDeclaration::Function(function) => function,
            other => panic!("expected function declaration, got {other:?}"),
        };

        match &function.body[0] {
            super::IrStatement::Return { value } => {
                assert_eq!(
                    value,
                    &Some(IrExpr::Compare {
                        left: Box::new(IrExpr::Binary {
                            left: Box::new(IrExpr::Reference(vec!["score".to_string()])),
                            op: IrBinaryOp::Add,
                            right: Box::new(IrExpr::Binary {
                                left: Box::new(IrExpr::Reference(vec!["bonus".to_string()])),
                                op: IrBinaryOp::Multiply,
                                right: Box::new(IrExpr::Integer(2)),
                            }),
                        }),
                        op: IrCompareOp::LessEqual,
                        right: Box::new(IrExpr::Reference(vec!["limit".to_string()])),
                    })
                );
            }
            other => panic!("expected return statement, got {other:?}"),
        }
    }
}
