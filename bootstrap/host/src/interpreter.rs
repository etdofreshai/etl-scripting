use crate::ast::{Declaration, FunctionDeclaration, SourceFile, Statement};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Integer(i64),
    Boolean(bool),
    Text(String),
    Record {
        type_name: String,
        fields: HashMap<String, Value>,
    },
    Void,
}

pub fn run_main(file: &SourceFile) -> Result<i64, String> {
    let interpreter = Interpreter::new(file);
    let result = interpreter.call_function("main", Vec::new())?;
    match result {
        Value::Integer(value) => Ok(value),
        Value::Void => Ok(0),
        other => Err(format!("main must return integer or void, got {other:?}")),
    }
}

struct Interpreter<'a> {
    file: &'a SourceFile,
    functions: HashMap<String, &'a FunctionDeclaration>,
}

impl<'a> Interpreter<'a> {
    fn new(file: &'a SourceFile) -> Self {
        let mut functions = HashMap::new();
        for declaration in &file.declarations {
            if let Declaration::Function(function) = declaration {
                functions.insert(function.name.clone(), function);
            }
        }
        Self { file, functions }
    }

    fn call_function(&self, name: &str, arguments: Vec<Value>) -> Result<Value, String> {
        let function = self
            .functions
            .get(name)
            .copied()
            .ok_or_else(|| format!("unknown function: {name}"))?;

        if function.parameters.len() != arguments.len() {
            return Err(format!(
                "function `{}` expected {} arguments, got {}",
                function.name,
                function.parameters.len(),
                arguments.len()
            ));
        }

        let mut scope = HashMap::new();
        for (parameter, argument) in function.parameters.iter().zip(arguments) {
            scope.insert(parameter.name.clone(), argument);
        }

        match self.execute_block(&function.body, &mut scope)? {
            ControlFlow::Return(value) => Ok(value),
            ControlFlow::Continue => Ok(Value::Void),
        }
    }

    fn execute_block(
        &self,
        statements: &[Statement],
        scope: &mut HashMap<String, Value>,
    ) -> Result<ControlFlow, String> {
        for statement in statements {
            match self.execute_statement(statement, scope)? {
                ControlFlow::Continue => continue,
                flow @ ControlFlow::Return(_) => return Ok(flow),
            }
        }
        Ok(ControlFlow::Continue)
    }

    fn execute_statement(
        &self,
        statement: &Statement,
        scope: &mut HashMap<String, Value>,
    ) -> Result<ControlFlow, String> {
        match statement {
            Statement::Let { name, value } => {
                let evaluated = self.evaluate_expression(value, scope)?;
                scope.insert(name.clone(), evaluated);
                Ok(ControlFlow::Continue)
            }
            Statement::Mutable { name, value, .. } => {
                let evaluated = self.evaluate_expression(value, scope)?;
                scope.insert(name.clone(), evaluated);
                Ok(ControlFlow::Continue)
            }
            Statement::Set { target, value } => {
                let evaluated = self.evaluate_expression(value, scope)?;
                scope.insert(target.clone(), evaluated);
                Ok(ControlFlow::Continue)
            }
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                if self.evaluate_expression(condition, scope)?.as_bool()? {
                    self.execute_block(then_body, scope)
                } else {
                    self.execute_block(else_body, scope)
                }
            }
            Statement::RepeatWhile { condition, body } => {
                while self.evaluate_expression(condition, scope)?.as_bool()? {
                    match self.execute_block(body, scope)? {
                        ControlFlow::Continue => {}
                        flow @ ControlFlow::Return(_) => return Ok(flow),
                    }
                }
                Ok(ControlFlow::Continue)
            }
            Statement::Return { value } => {
                let evaluated = match value {
                    Some(expression) => self.evaluate_expression(expression, scope)?,
                    None => Value::Void,
                };
                Ok(ControlFlow::Return(evaluated))
            }
            Statement::Expression { value } => {
                let _ = self.evaluate_expression(value, scope)?;
                Ok(ControlFlow::Continue)
            }
        }
    }

    fn evaluate_expression(
        &self,
        expression: &str,
        scope: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let tokens = ExprLexer::new(expression).tokenize()?;
        let mut parser = ExprParser::new(tokens, self, scope);
        parser.parse_expression()
    }

    fn call_named(&self, name: &str, arguments: Vec<Value>) -> Result<Value, String> {
        match name {
            "io.print_line" => {
                let text = arguments
                    .first()
                    .ok_or_else(|| "io.print_line requires one argument".to_string())?
                    .as_text()?;
                println!("{text}");
                Ok(Value::Void)
            }
            _ => self.call_function(name, arguments),
        }
    }

    fn make_record(&self, type_name: &str, pairs: Vec<(String, Value)>) -> Result<Value, String> {
        let declaration = self
            .file
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                Declaration::Record(record) if record.name == type_name => Some(record),
                _ => None,
            })
            .ok_or_else(|| format!("unknown record type: {type_name}"))?;

        let mut fields = HashMap::new();
        for field in &declaration.fields {
            let value = pairs
                .iter()
                .find_map(|(name, value)| (name == &field.name).then(|| value.clone()))
                .ok_or_else(|| {
                    format!("missing field `{}` for record `{}`", field.name, type_name)
                })?;
            fields.insert(field.name.clone(), value);
        }

        Ok(Value::Record {
            type_name: type_name.to_string(),
            fields,
        })
    }
}

enum ControlFlow {
    Continue,
    Return(Value),
}

impl Value {
    fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Boolean(value) => Ok(*value),
            other => Err(format!("expected boolean, got {other:?}")),
        }
    }

    fn as_text(&self) -> Result<String, String> {
        match self {
            Value::Text(value) => Ok(value.clone()),
            other => Err(format!("expected text, got {other:?}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExprToken {
    Identifier(String),
    Integer(i64),
    Text(String),
    True,
    False,
    And,
    Or,
    LeftParen,
    RightParen,
    Comma,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    EqualEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
}

struct ExprLexer<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> ExprLexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    fn tokenize(mut self) -> Result<Vec<ExprToken>, String> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek() {
            match ch {
                ' ' => self.index += 1,
                'a'..='z' | 'A'..='Z' | '_' => tokens.push(self.identifier()),
                '0'..='9' => tokens.push(self.integer()?),
                '"' => tokens.push(self.text()?),
                '(' => {
                    self.index += 1;
                    tokens.push(ExprToken::LeftParen);
                }
                ')' => {
                    self.index += 1;
                    tokens.push(ExprToken::RightParen);
                }
                ',' => {
                    self.index += 1;
                    tokens.push(ExprToken::Comma);
                }
                '.' => {
                    self.index += 1;
                    tokens.push(ExprToken::Dot);
                }
                '+' => {
                    self.index += 1;
                    tokens.push(ExprToken::Plus);
                }
                '-' => {
                    self.index += 1;
                    tokens.push(ExprToken::Minus);
                }
                '*' => {
                    self.index += 1;
                    tokens.push(ExprToken::Star);
                }
                '/' => {
                    self.index += 1;
                    tokens.push(ExprToken::Slash);
                }
                '<' => {
                    self.index += 1;
                    if self.peek() == Some('=') {
                        self.index += 1;
                        tokens.push(ExprToken::LessEqual);
                    } else {
                        tokens.push(ExprToken::Less);
                    }
                }
                '>' => {
                    self.index += 1;
                    if self.peek() == Some('=') {
                        self.index += 1;
                        tokens.push(ExprToken::GreaterEqual);
                    } else {
                        tokens.push(ExprToken::Greater);
                    }
                }
                '=' => {
                    self.index += 1;
                    if self.peek() == Some('=') {
                        self.index += 1;
                        tokens.push(ExprToken::EqualEqual);
                    } else {
                        return Err("single `=` is not supported in expressions".to_string());
                    }
                }
                _ => return Err(format!("unexpected expression character: {ch}")),
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.input[self.index..].chars().next()
    }

    fn identifier(&mut self) -> ExprToken {
        let start = self.index;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.index += ch.len_utf8();
            } else {
                break;
            }
        }
        match &self.input[start..self.index] {
            "true" => ExprToken::True,
            "false" => ExprToken::False,
            "and" => ExprToken::And,
            "or" => ExprToken::Or,
            other => ExprToken::Identifier(other.to_string()),
        }
    }

    fn integer(&mut self) -> Result<ExprToken, String> {
        let start = self.index;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.index += ch.len_utf8();
            } else {
                break;
            }
        }
        self.input[start..self.index]
            .parse()
            .map(ExprToken::Integer)
            .map_err(|error| format!("invalid integer literal: {error}"))
    }

    fn text(&mut self) -> Result<ExprToken, String> {
        self.index += 1;
        let start = self.index;
        while let Some(ch) = self.peek() {
            if ch == '"' {
                let value = self.input[start..self.index].to_string();
                self.index += 1;
                return Ok(ExprToken::Text(value));
            }
            self.index += ch.len_utf8();
        }
        Err("unterminated string literal".to_string())
    }
}

struct ExprParser<'a, 'b> {
    tokens: Vec<ExprToken>,
    index: usize,
    interpreter: &'b Interpreter<'a>,
    scope: &'b HashMap<String, Value>,
}

impl<'a, 'b> ExprParser<'a, 'b> {
    fn new(
        tokens: Vec<ExprToken>,
        interpreter: &'b Interpreter<'a>,
        scope: &'b HashMap<String, Value>,
    ) -> Self {
        Self {
            tokens,
            index: 0,
            interpreter,
            scope,
        }
    }

    fn parse_expression(&mut self) -> Result<Value, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Value, String> {
        let mut left = self.parse_and()?;
        while self.match_token(&ExprToken::Or) {
            let right = self.parse_and()?;
            left = Value::Boolean(left.as_bool()? || right.as_bool()?);
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Value, String> {
        let mut left = self.parse_comparison()?;
        while self.match_token(&ExprToken::And) {
            let right = self.parse_comparison()?;
            left = Value::Boolean(left.as_bool()? && right.as_bool()?);
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Value, String> {
        let mut left = self.parse_term()?;
        loop {
            if self.match_token(&ExprToken::EqualEqual) {
                let right = self.parse_term()?;
                left = Value::Boolean(left == right);
            } else if self.match_token(&ExprToken::Less) {
                let right = self.parse_term()?;
                left = Value::Boolean(left.as_int()? < right.as_int()?);
            } else if self.match_token(&ExprToken::Greater) {
                let right = self.parse_term()?;
                left = Value::Boolean(left.as_int()? > right.as_int()?);
            } else if self.match_token(&ExprToken::LessEqual) {
                let right = self.parse_term()?;
                left = Value::Boolean(left.as_int()? <= right.as_int()?);
            } else if self.match_token(&ExprToken::GreaterEqual) {
                let right = self.parse_term()?;
                left = Value::Boolean(left.as_int()? >= right.as_int()?);
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Value, String> {
        let mut left = self.parse_factor()?;
        loop {
            if self.match_token(&ExprToken::Plus) {
                let right = self.parse_factor()?;
                left = Value::Integer(left.as_int()? + right.as_int()?);
            } else if self.match_token(&ExprToken::Minus) {
                let right = self.parse_factor()?;
                left = Value::Integer(left.as_int()? - right.as_int()?);
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Value, String> {
        let mut left = self.parse_primary()?;
        loop {
            if self.match_token(&ExprToken::Star) {
                let right = self.parse_primary()?;
                left = Value::Integer(left.as_int()? * right.as_int()?);
            } else if self.match_token(&ExprToken::Slash) {
                let right = self.parse_primary()?;
                left = Value::Integer(left.as_int()? / right.as_int()?);
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Value, String> {
        match self.advance().cloned() {
            Some(ExprToken::Integer(value)) => Ok(Value::Integer(value)),
            Some(ExprToken::Text(value)) => Ok(Value::Text(value)),
            Some(ExprToken::True) => Ok(Value::Boolean(true)),
            Some(ExprToken::False) => Ok(Value::Boolean(false)),
            Some(ExprToken::Identifier(name)) => self.parse_identifier_expression(name),
            Some(ExprToken::LeftParen) => {
                let value = self.parse_expression()?;
                self.expect(&ExprToken::RightParen)?;
                Ok(value)
            }
            other => Err(format!("unexpected expression token: {other:?}")),
        }
    }

    fn parse_identifier_expression(&mut self, first: String) -> Result<Value, String> {
        let mut name = first;
        while self.match_token(&ExprToken::Dot) {
            let next = self.expect_identifier()?;
            name.push('.');
            name.push_str(&next);
        }

        if self.match_token(&ExprToken::LeftParen) {
            let args = self.parse_call_arguments()?;
            return self.interpreter.call_named(&name, args);
        }

        if name.contains('.') {
            let mut segments = name.split('.');
            let base = segments.next().unwrap();
            let mut value = self
                .scope
                .get(base)
                .cloned()
                .ok_or_else(|| format!("unknown variable: {base}"))?;
            for field in segments {
                value = value.field(field)?;
            }
            return Ok(value);
        }

        self.scope
            .get(&name)
            .cloned()
            .ok_or_else(|| format!("unknown variable: {name}"))
    }

    fn parse_call_arguments(&mut self) -> Result<Vec<Value>, String> {
        if self.match_token(&ExprToken::RightParen) {
            return Ok(Vec::new());
        }

        let mut values = Vec::new();
        loop {
            values.push(self.parse_expression()?);
            if self.match_token(&ExprToken::Comma) {
                continue;
            }
            self.expect(&ExprToken::RightParen)?;
            break;
        }
        Ok(values)
    }

    fn expect_identifier(&mut self) -> Result<String, String> {
        match self.advance().cloned() {
            Some(ExprToken::Identifier(name)) => Ok(name),
            other => Err(format!("expected identifier, got {other:?}")),
        }
    }

    fn expect(&mut self, token: &ExprToken) -> Result<(), String> {
        if self.match_token(token) {
            Ok(())
        } else {
            Err(format!("expected token {token:?}"))
        }
    }

    fn match_token(&mut self, token: &ExprToken) -> bool {
        if self.peek() == Some(token) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<&ExprToken> {
        self.tokens.get(self.index)
    }

    fn advance(&mut self) -> Option<&ExprToken> {
        let token = self.tokens.get(self.index);
        if token.is_some() {
            self.index += 1;
        }
        token
    }
}

impl Value {
    fn as_int(&self) -> Result<i64, String> {
        match self {
            Value::Integer(value) => Ok(*value),
            other => Err(format!("expected integer, got {other:?}")),
        }
    }

    fn field(&self, name: &str) -> Result<Value, String> {
        match self {
            Value::Record { fields, .. } => fields
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown field: {name}")),
            other => Err(format!("expected record for field access, got {other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run_main;
    use crate::parser::parse_source;
    use crate::typecheck::validate_source_file;

    #[test]
    fn runs_hello_world_main() {
        let source = r#"module demo.hello_world

import standard.io

define function main returns integer
    io.print_line("Hello from ETL")
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let exit_code = run_main(&file).expect("program should run");
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn runs_counter_loop_main() {
        let source = r#"module demo.counter_loop

define function main returns integer
    mutable current as integer be 0

    repeat while current < 3
        set current to current + 1

    return current
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let exit_code = run_main(&file).expect("program should run");
        assert_eq!(exit_code, 3);
    }
}
