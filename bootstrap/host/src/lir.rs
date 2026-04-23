use crate::ir::{
    IrBinaryOp, IrCompareOp, IrDeclaration, IrExpr, IrLogicalOp, IrProgram, IrStatement,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearProgram {
    pub functions: Vec<LinearFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearFunction {
    pub name: String,
    pub instructions: Vec<LinearInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinearInstruction {
    LoadInteger(i64),
    LoadBoolean(bool),
    LoadText(String),
    LoadReference(Vec<String>),
    ConstructRecord {
        type_name: String,
        field_count: usize,
    },
    Call {
        callee: Vec<String>,
        argument_count: usize,
    },
    Add,
    Subtract,
    Multiply,
    Divide,
    CompareEqual,
    CompareLess,
    CompareGreater,
    CompareLessEqual,
    CompareGreaterEqual,
    LogicalAnd,
    LogicalOr,
    StoreLocal(String),
    StoreReference(Vec<String>),
    Label(String),
    Jump(String),
    JumpIfFalse(String),
    Pop,
    Return,
}

pub fn lower_program(program: &IrProgram) -> Result<LinearProgram, String> {
    let functions = program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            IrDeclaration::Function(function) => Some(lower_function(function)),
            IrDeclaration::Record(_) => None,
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LinearProgram { functions })
}

struct LoweringContext {
    function_name: String,
    next_label_id: usize,
}

impl LoweringContext {
    fn new(function_name: &str) -> Self {
        Self {
            function_name: function_name.to_string(),
            next_label_id: 0,
        }
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}_{}", self.function_name, prefix, self.next_label_id);
        self.next_label_id += 1;
        label
    }
}

fn lower_function(function: &crate::ir::IrFunction) -> Result<LinearFunction, String> {
    let mut instructions = Vec::new();
    let mut context = LoweringContext::new(&function.name);
    for statement in &function.body {
        lower_statement(statement, &mut instructions, &mut context)?;
    }

    Ok(LinearFunction {
        name: function.name.clone(),
        instructions,
    })
}

fn lower_statement(
    statement: &IrStatement,
    instructions: &mut Vec<LinearInstruction>,
    context: &mut LoweringContext,
) -> Result<(), String> {
    match statement {
        IrStatement::Let { name, value } | IrStatement::Mutable { name, value, .. } => {
            lower_expression(value, instructions)?;
            instructions.push(LinearInstruction::StoreLocal(name.clone()));
        }
        IrStatement::Set { target, value } => {
            lower_expression(value, instructions)?;
            instructions.push(LinearInstruction::StoreReference(target.clone()));
        }
        IrStatement::If {
            condition,
            then_body,
            else_body,
        } => {
            let else_label = context.fresh_label("if_else");
            let end_label = context.fresh_label("if_end");

            lower_expression(condition, instructions)?;
            instructions.push(LinearInstruction::JumpIfFalse(else_label.clone()));
            for nested in then_body {
                lower_statement(nested, instructions, context)?;
            }
            instructions.push(LinearInstruction::Jump(end_label.clone()));
            instructions.push(LinearInstruction::Label(else_label));
            for nested in else_body {
                lower_statement(nested, instructions, context)?;
            }
            instructions.push(LinearInstruction::Label(end_label));
        }
        IrStatement::RepeatWhile { condition, body } => {
            let start_label = context.fresh_label("loop_start");
            let end_label = context.fresh_label("loop_end");

            instructions.push(LinearInstruction::Label(start_label.clone()));
            lower_expression(condition, instructions)?;
            instructions.push(LinearInstruction::JumpIfFalse(end_label.clone()));
            for nested in body {
                lower_statement(nested, instructions, context)?;
            }
            instructions.push(LinearInstruction::Jump(start_label));
            instructions.push(LinearInstruction::Label(end_label));
        }
        IrStatement::Return { value } => {
            if let Some(value) = value {
                lower_expression(value, instructions)?;
            }
            instructions.push(LinearInstruction::Return);
        }
        IrStatement::Expr { value } => {
            lower_expression(value, instructions)?;
            instructions.push(LinearInstruction::Pop);
        }
    }

    Ok(())
}

fn lower_expression(
    expression: &IrExpr,
    instructions: &mut Vec<LinearInstruction>,
) -> Result<(), String> {
    match expression {
        IrExpr::Integer(value) => instructions.push(LinearInstruction::LoadInteger(*value)),
        IrExpr::Boolean(value) => instructions.push(LinearInstruction::LoadBoolean(*value)),
        IrExpr::Text(value) => instructions.push(LinearInstruction::LoadText(value.clone())),
        IrExpr::Reference(path) => {
            instructions.push(LinearInstruction::LoadReference(path.clone()))
        }
        IrExpr::Binary { left, op, right } => {
            lower_expression(left, instructions)?;
            lower_expression(right, instructions)?;
            instructions.push(match op {
                IrBinaryOp::Add => LinearInstruction::Add,
                IrBinaryOp::Subtract => LinearInstruction::Subtract,
                IrBinaryOp::Multiply => LinearInstruction::Multiply,
                IrBinaryOp::Divide => LinearInstruction::Divide,
            });
        }
        IrExpr::Compare { left, op, right } => {
            lower_expression(left, instructions)?;
            lower_expression(right, instructions)?;
            instructions.push(match op {
                IrCompareOp::Equal => LinearInstruction::CompareEqual,
                IrCompareOp::Less => LinearInstruction::CompareLess,
                IrCompareOp::Greater => LinearInstruction::CompareGreater,
                IrCompareOp::LessEqual => LinearInstruction::CompareLessEqual,
                IrCompareOp::GreaterEqual => LinearInstruction::CompareGreaterEqual,
            });
        }
        IrExpr::Logical { left, op, right } => {
            lower_expression(left, instructions)?;
            lower_expression(right, instructions)?;
            instructions.push(match op {
                IrLogicalOp::And => LinearInstruction::LogicalAnd,
                IrLogicalOp::Or => LinearInstruction::LogicalOr,
            });
        }
        IrExpr::RecordConstruct { type_name, fields } => {
            for (_, value) in fields {
                lower_expression(value, instructions)?;
            }
            instructions.push(LinearInstruction::ConstructRecord {
                type_name: type_name.clone(),
                field_count: fields.len(),
            });
        }
        IrExpr::Call { callee, arguments } => {
            for argument in arguments {
                lower_expression(argument, instructions)?;
            }
            instructions.push(LinearInstruction::Call {
                callee: callee.clone(),
                argument_count: arguments.len(),
            });
        }
        IrExpr::Opaque(source) => {
            return Err(format!(
                "opaque expression lowering not implemented yet: {source}"
            ))
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{lower_program, LinearFunction, LinearInstruction, LinearProgram};
    use crate::ir::lower_source_file;
    use crate::parser::parse_source;
    use crate::typecheck::validate_source_file;

    #[test]
    fn lowers_return_expression_to_linear_stack_code() {
        let source = r#"module demo.lir

define function main takes score as integer, bonus as integer, limit as integer, ready as boolean returns boolean
    return score + bonus * 2 <= limit and ready
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");

        assert_eq!(
            linear,
            LinearProgram {
                functions: vec![LinearFunction {
                    name: "main".to_string(),
                    instructions: vec![
                        LinearInstruction::LoadReference(vec!["score".to_string()]),
                        LinearInstruction::LoadReference(vec!["bonus".to_string()]),
                        LinearInstruction::LoadInteger(2),
                        LinearInstruction::Multiply,
                        LinearInstruction::Add,
                        LinearInstruction::LoadReference(vec!["limit".to_string()]),
                        LinearInstruction::CompareLessEqual,
                        LinearInstruction::LoadReference(vec!["ready".to_string()]),
                        LinearInstruction::LogicalAnd,
                        LinearInstruction::Return,
                    ],
                }],
            }
        );
    }

    #[test]
    fn lowers_if_else_to_labels_and_jumps() {
        let source = r#"module demo.lir

define function main takes ready as boolean returns integer
    if ready
        return 1
    else
        return 2
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");

        assert_eq!(
            linear,
            LinearProgram {
                functions: vec![LinearFunction {
                    name: "main".to_string(),
                    instructions: vec![
                        LinearInstruction::LoadReference(vec!["ready".to_string()]),
                        LinearInstruction::JumpIfFalse("main_if_else_0".to_string()),
                        LinearInstruction::LoadInteger(1),
                        LinearInstruction::Return,
                        LinearInstruction::Jump("main_if_end_1".to_string()),
                        LinearInstruction::Label("main_if_else_0".to_string()),
                        LinearInstruction::LoadInteger(2),
                        LinearInstruction::Return,
                        LinearInstruction::Label("main_if_end_1".to_string()),
                    ],
                }],
            }
        );
    }

    #[test]
    fn lowers_repeat_while_to_loop_labels_and_jumps() {
        let source = r#"module demo.lir

define function main takes ready as boolean returns integer
    mutable score as integer be 0
    repeat while ready
        set score to score + 1
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");

        assert_eq!(
            linear,
            LinearProgram {
                functions: vec![LinearFunction {
                    name: "main".to_string(),
                    instructions: vec![
                        LinearInstruction::LoadInteger(0),
                        LinearInstruction::StoreLocal("score".to_string()),
                        LinearInstruction::Label("main_loop_start_0".to_string()),
                        LinearInstruction::LoadReference(vec!["ready".to_string()]),
                        LinearInstruction::JumpIfFalse("main_loop_end_1".to_string()),
                        LinearInstruction::LoadReference(vec!["score".to_string()]),
                        LinearInstruction::LoadInteger(1),
                        LinearInstruction::Add,
                        LinearInstruction::StoreReference(vec!["score".to_string()]),
                        LinearInstruction::Jump("main_loop_start_0".to_string()),
                        LinearInstruction::Label("main_loop_end_1".to_string()),
                        LinearInstruction::LoadReference(vec!["score".to_string()]),
                        LinearInstruction::Return,
                    ],
                }],
            }
        );
    }
}
