use crate::ast::{Declaration, FunctionDeclaration, SourceFile, Statement};
use std::collections::{HashMap, HashSet};

const BUILTIN_TYPES: &[&str] = &["integer", "float", "boolean", "text", "void"];

#[derive(Clone)]
struct FunctionSignature {
    parameter_types: Vec<String>,
    return_type: String,
}

pub fn validate_source_file(file: &SourceFile) -> Result<(), String> {
    let mut known_types: HashSet<String> = BUILTIN_TYPES
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    known_types.insert("standard.random.generator".to_string());
    let mut record_fields: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut functions = builtin_functions();
    let mut seen_declarations = HashSet::new();

    for declaration in &file.declarations {
        match declaration {
            Declaration::Record(record) => {
                if !seen_declarations.insert(format!("record:{}", record.name)) {
                    return Err(format!("duplicate record declaration: {}", record.name));
                }
                known_types.insert(record.name.clone());
                record_fields.insert(
                    record.name.clone(),
                    record
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), field.field_type.name.clone()))
                        .collect(),
                );
            }
            Declaration::Function(function) => {
                if !seen_declarations.insert(format!("function:{}", function.name)) {
                    return Err(format!("duplicate function declaration: {}", function.name));
                }
                functions.insert(
                    function.name.clone(),
                    FunctionSignature {
                        parameter_types: function
                            .parameters
                            .iter()
                            .map(|parameter| parameter.parameter_type.name.clone())
                            .collect(),
                        return_type: function.return_type.name.clone(),
                    },
                );
            }
        }
    }

    for declaration in &file.declarations {
        match declaration {
            Declaration::Record(record) => {
                for field in &record.fields {
                    ensure_known_type(&known_types, &field.field_type.name)?;
                }
            }
            Declaration::Function(function) => {
                validate_function(function, &known_types, &record_fields, &functions)?;
            }
        }
    }

    Ok(())
}

fn validate_function(
    function: &FunctionDeclaration,
    known_types: &HashSet<String>,
    record_fields: &HashMap<String, HashMap<String, String>>,
    functions: &HashMap<String, FunctionSignature>,
) -> Result<(), String> {
    let mut scope = HashMap::new();
    for parameter in &function.parameters {
        ensure_known_type(known_types, &parameter.parameter_type.name)?;
        scope.insert(
            parameter.name.clone(),
            parameter.parameter_type.name.clone(),
        );
    }
    ensure_known_type(known_types, &function.return_type.name)?;
    validate_statements(
        known_types,
        record_fields,
        functions,
        &function.name,
        &function.return_type.name,
        &mut scope,
        &function.body,
    )
}

fn validate_statements(
    known_types: &HashSet<String>,
    record_fields: &HashMap<String, HashMap<String, String>>,
    functions: &HashMap<String, FunctionSignature>,
    function_name: &str,
    function_return_type: &str,
    scope: &mut HashMap<String, String>,
    statements: &[Statement],
) -> Result<(), String> {
    for statement in statements {
        match statement {
            Statement::Let { name, value } => {
                let value_type =
                    validate_expression(value, scope, known_types, record_fields, functions)?;
                scope.insert(name.clone(), value_type);
            }
            Statement::Mutable {
                name,
                value_type,
                value,
            } => {
                ensure_known_type(known_types, &value_type.name)?;
                let actual_type =
                    validate_expression(value, scope, known_types, record_fields, functions)?;
                ensure_type_matches(
                    &value_type.name,
                    &actual_type,
                    &format!("variable `{name}` declared as"),
                )?;
                scope.insert(name.clone(), value_type.name.clone());
            }
            Statement::Set { target, value } => {
                let target_type = validate_reference(target, scope, record_fields)?;
                let value_type =
                    validate_expression(value, scope, known_types, record_fields, functions)?;
                if target_type != value_type {
                    return Err(format!(
                        "cannot assign `{value_type}` to `{target}` of type `{target_type}`"
                    ));
                }
            }
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                let condition_type =
                    validate_expression(condition, scope, known_types, record_fields, functions)?;
                ensure_type_matches("boolean", &condition_type, "if condition must be")?;
                validate_statements(
                    known_types,
                    record_fields,
                    functions,
                    function_name,
                    function_return_type,
                    scope,
                    then_body,
                )?;
                validate_statements(
                    known_types,
                    record_fields,
                    functions,
                    function_name,
                    function_return_type,
                    scope,
                    else_body,
                )?;
            }
            Statement::RepeatWhile { condition, body } => {
                let condition_type =
                    validate_expression(condition, scope, known_types, record_fields, functions)?;
                ensure_type_matches("boolean", &condition_type, "repeat while condition must be")?;
                validate_statements(
                    known_types,
                    record_fields,
                    functions,
                    function_name,
                    function_return_type,
                    scope,
                    body,
                )?;
            }
            Statement::Return { value } => {
                validate_return(
                    function_name,
                    function_return_type,
                    value.as_deref(),
                    scope,
                    known_types,
                    record_fields,
                    functions,
                )?;
            }
            Statement::Expression { value } => {
                validate_expression(value, scope, known_types, record_fields, functions)?;
            }
        }
    }

    Ok(())
}

fn validate_return(
    function_name: &str,
    function_return_type: &str,
    value: Option<&str>,
    scope: &HashMap<String, String>,
    known_types: &HashSet<String>,
    record_fields: &HashMap<String, HashMap<String, String>>,
    functions: &HashMap<String, FunctionSignature>,
) -> Result<(), String> {
    match (function_return_type == "void", value) {
        (true, Some(_)) => Err(format!(
            "function `{function_name}` cannot return a value from a void function"
        )),
        (false, None) => Err(format!(
            "function `{function_name}` must return a value of type `{function_return_type}`"
        )),
        (_, Some(expression)) => {
            let actual_type =
                validate_expression(expression, scope, known_types, record_fields, functions)?;
            ensure_type_matches(
                function_return_type,
                &actual_type,
                &format!("function `{function_name}` must return"),
            )
        }
        (_, None) => Ok(()),
    }
}

fn validate_expression(
    expression: &str,
    scope: &HashMap<String, String>,
    known_types: &HashSet<String>,
    record_fields: &HashMap<String, HashMap<String, String>>,
    functions: &HashMap<String, FunctionSignature>,
) -> Result<String, String> {
    let tokens = ExprLexer::new(expression).tokenize()?;
    let mut parser = ExprParser::new(tokens, scope, known_types, record_fields, functions);
    let expression_type = parser.parse_expression()?;
    parser.ensure_complete()?;
    Ok(expression_type)
}

fn validate_reference(
    reference: &str,
    scope: &HashMap<String, String>,
    record_fields: &HashMap<String, HashMap<String, String>>,
) -> Result<String, String> {
    let mut segments = reference.split('.');
    let base = segments.next().unwrap_or(reference);
    let mut current_type = scope
        .get(base)
        .cloned()
        .ok_or_else(|| format!("unknown variable: {base}"))?;

    for field_name in segments {
        let fields = record_fields
            .get(&current_type)
            .ok_or_else(|| format!("type `{current_type}` has no fields"))?;
        current_type = fields
            .get(field_name)
            .cloned()
            .ok_or_else(|| format!("unknown field `{field_name}` for record `{current_type}`"))?;
    }

    Ok(current_type)
}

fn ensure_known_type(known_types: &HashSet<String>, type_name: &str) -> Result<(), String> {
    if known_types.contains(type_name) {
        Ok(())
    } else {
        Err(format!("unknown type: {type_name}"))
    }
}

fn ensure_type_matches(expected: &str, actual: &str, context: &str) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!("{context} `{expected}`, got `{actual}`"))
    }
}

fn builtin_functions() -> HashMap<String, FunctionSignature> {
    HashMap::from([
        (
            "io.print_line".to_string(),
            FunctionSignature {
                parameter_types: vec!["text".to_string()],
                return_type: "void".to_string(),
            },
        ),
        (
            "random.from_seed".to_string(),
            FunctionSignature {
                parameter_types: vec!["integer".to_string()],
                return_type: "standard.random.generator".to_string(),
            },
        ),
        (
            "random.next_integer".to_string(),
            FunctionSignature {
                parameter_types: vec![
                    "standard.random.generator".to_string(),
                    "integer".to_string(),
                    "integer".to_string(),
                ],
                return_type: "integer".to_string(),
            },
        ),
        (
            "event.push_hit".to_string(),
            FunctionSignature {
                parameter_types: vec!["integer".to_string(), "integer".to_string()],
                return_type: "void".to_string(),
            },
        ),
    ])
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

struct ExprParser<'a> {
    tokens: Vec<ExprToken>,
    index: usize,
    scope: &'a HashMap<String, String>,
    known_types: &'a HashSet<String>,
    record_fields: &'a HashMap<String, HashMap<String, String>>,
    functions: &'a HashMap<String, FunctionSignature>,
}

impl<'a> ExprParser<'a> {
    fn new(
        tokens: Vec<ExprToken>,
        scope: &'a HashMap<String, String>,
        known_types: &'a HashSet<String>,
        record_fields: &'a HashMap<String, HashMap<String, String>>,
        functions: &'a HashMap<String, FunctionSignature>,
    ) -> Self {
        Self {
            tokens,
            index: 0,
            scope,
            known_types,
            record_fields,
            functions,
        }
    }

    fn parse_expression(&mut self) -> Result<String, String> {
        self.parse_or()
    }

    fn ensure_complete(&self) -> Result<(), String> {
        if let Some(token) = self.tokens.get(self.index) {
            Err(format!("unexpected expression token: {token:?}"))
        } else {
            Ok(())
        }
    }

    fn parse_or(&mut self) -> Result<String, String> {
        let mut left = self.parse_and()?;
        while self.match_token(&ExprToken::Or) {
            ensure_type_matches("boolean", &left, "logical `or` left operand must be")?;
            let right = self.parse_and()?;
            ensure_type_matches("boolean", &right, "logical `or` right operand must be")?;
            left = "boolean".to_string();
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<String, String> {
        let mut left = self.parse_comparison()?;
        while self.match_token(&ExprToken::And) {
            ensure_type_matches("boolean", &left, "logical `and` left operand must be")?;
            let right = self.parse_comparison()?;
            ensure_type_matches("boolean", &right, "logical `and` right operand must be")?;
            left = "boolean".to_string();
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<String, String> {
        let mut left = self.parse_term()?;
        loop {
            if self.match_token(&ExprToken::EqualEqual) {
                let right = self.parse_term()?;
                if left != right {
                    return Err(format!(
                        "equality operands must have matching types, got `{left}` and `{right}`"
                    ));
                }
                left = "boolean".to_string();
            } else if self.match_token(&ExprToken::Less)
                || self.match_token(&ExprToken::Greater)
                || self.match_token(&ExprToken::LessEqual)
                || self.match_token(&ExprToken::GreaterEqual)
            {
                let right = self.parse_term()?;
                ensure_type_matches("integer", &left, "comparison left operand must be")?;
                ensure_type_matches("integer", &right, "comparison right operand must be")?;
                left = "boolean".to_string();
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<String, String> {
        let mut left = self.parse_factor()?;
        loop {
            if self.match_token(&ExprToken::Plus) || self.match_token(&ExprToken::Minus) {
                let right = self.parse_factor()?;
                ensure_type_matches("integer", &left, "arithmetic left operand must be")?;
                ensure_type_matches("integer", &right, "arithmetic right operand must be")?;
                left = "integer".to_string();
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<String, String> {
        let mut left = self.parse_primary()?;
        loop {
            if self.match_token(&ExprToken::Star) || self.match_token(&ExprToken::Slash) {
                let right = self.parse_primary()?;
                ensure_type_matches("integer", &left, "arithmetic left operand must be")?;
                ensure_type_matches("integer", &right, "arithmetic right operand must be")?;
                left = "integer".to_string();
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<String, String> {
        match self.advance().cloned() {
            Some(ExprToken::Integer(_)) => Ok("integer".to_string()),
            Some(ExprToken::Text(_)) => Ok("text".to_string()),
            Some(ExprToken::True) | Some(ExprToken::False) => Ok("boolean".to_string()),
            Some(ExprToken::Identifier(name)) => self.parse_identifier_expression(name),
            Some(ExprToken::LeftParen) => {
                let value_type = self.parse_expression()?;
                self.expect(&ExprToken::RightParen)?;
                Ok(value_type)
            }
            other => Err(format!("unexpected expression token: {other:?}")),
        }
    }

    fn parse_identifier_expression(&mut self, first: String) -> Result<String, String> {
        let mut name = first;
        while self.match_token(&ExprToken::Dot) {
            let next = self.expect_identifier()?;
            name.push('.');
            name.push_str(&next);
        }

        if self.match_token(&ExprToken::LeftParen) {
            if !name.contains('.') && self.known_types.contains(&name) {
                return self.parse_record_constructor(&name);
            }
            return self.parse_call(&name);
        }

        validate_reference(&name, self.scope, self.record_fields)
    }

    fn parse_call(&mut self, name: &str) -> Result<String, String> {
        let argument_types = self.parse_call_arguments()?;
        let signature = self
            .functions
            .get(name)
            .ok_or_else(|| format!("unknown function: {name}"))?;
        if signature.parameter_types.len() != argument_types.len() {
            return Err(format!(
                "function `{name}` expects {} arguments, got {}",
                signature.parameter_types.len(),
                argument_types.len()
            ));
        }
        for (index, (expected, actual)) in signature
            .parameter_types
            .iter()
            .zip(argument_types.iter())
            .enumerate()
        {
            ensure_type_matches(
                expected,
                actual,
                &format!("function `{name}` argument {} must be", index + 1),
            )?;
        }
        Ok(signature.return_type.clone())
    }

    fn parse_record_constructor(&mut self, type_name: &str) -> Result<String, String> {
        let known_fields = self
            .record_fields
            .get(type_name)
            .ok_or_else(|| format!("unknown record type: {type_name}"))?;
        let mut seen_fields = HashSet::new();

        if self.match_token(&ExprToken::RightParen) {
            return self.finish_record_constructor(type_name, known_fields, seen_fields);
        }

        loop {
            let field_name = self.expect_identifier()?;
            let field_type = known_fields
                .get(&field_name)
                .cloned()
                .ok_or_else(|| format!("unknown field `{field_name}` for record `{type_name}`"))?;
            let value_type = self.parse_expression()?;
            ensure_type_matches(
                &field_type,
                &value_type,
                &format!("record field `{field_name}` for `{type_name}` must be"),
            )?;
            seen_fields.insert(field_name);
            if self.match_token(&ExprToken::Comma) {
                continue;
            }
            self.expect(&ExprToken::RightParen)?;
            break;
        }

        self.finish_record_constructor(type_name, known_fields, seen_fields)
    }

    fn finish_record_constructor(
        &self,
        type_name: &str,
        known_fields: &HashMap<String, String>,
        seen_fields: HashSet<String>,
    ) -> Result<String, String> {
        for field_name in known_fields.keys() {
            if !seen_fields.contains(field_name) {
                return Err(format!(
                    "missing field `{field_name}` for record `{type_name}`"
                ));
            }
        }
        Ok(type_name.to_string())
    }

    fn parse_call_arguments(&mut self) -> Result<Vec<String>, String> {
        if self.match_token(&ExprToken::RightParen) {
            return Ok(Vec::new());
        }

        let mut argument_types = Vec::new();
        loop {
            argument_types.push(self.parse_expression()?);
            if self.match_token(&ExprToken::Comma) {
                continue;
            }
            self.expect(&ExprToken::RightParen)?;
            break;
        }
        Ok(argument_types)
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

#[cfg(test)]
mod tests {
    use super::validate_source_file;
    use crate::parser::parse_source;

    #[test]
    fn accepts_builtin_and_record_types() {
        let source = r#"module game.types

define record entity_state
    health as integer


define function make_entity returns entity_state
    return entity_state(health 10)
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("types should validate");
    }

    #[test]
    fn rejects_unknown_types() {
        let source = r#"module game.types

define function broken takes value as mystery_type returns integer
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("unknown type should fail");
        assert!(error.contains("mystery_type"));
    }

    #[test]
    fn rejects_unknown_variables_in_return_expressions() {
        let source = r#"module demo.unknown_variable

define function main returns integer
    return missing_value
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("unknown variable should fail");
        assert!(error.contains("unknown variable: missing_value"));
    }

    #[test]
    fn rejects_calls_to_unknown_local_functions() {
        let source = r#"module demo.unknown_function

define function main returns integer
    return compute_score(1)
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("unknown function should fail");
        assert!(error.contains("unknown function: compute_score"));
    }

    #[test]
    fn rejects_local_function_calls_with_wrong_arity() {
        let source = r#"module demo.bad_arity

define function add takes left as integer, right as integer returns integer
    return left + right

define function main returns integer
    return add(1)
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("arity mismatch should fail");
        assert!(error.contains("function `add` expects 2 arguments, got 1"));
    }

    #[test]
    fn rejects_return_value_with_wrong_type() {
        let source = r#"module demo.bad_return_type

define function main returns integer
    return true
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("wrong return type should fail");
        assert!(error.contains("function `main` must return `integer`, got `boolean`"));
    }

    #[test]
    fn rejects_non_boolean_if_conditions() {
        let source = r#"module demo.bad_if_condition

define function main returns integer
    if 1
        return 1

    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("non-boolean condition should fail");
        assert!(error.contains("if condition must be `boolean`, got `integer`"));
    }

    #[test]
    fn rejects_mutable_initializer_with_wrong_type() {
        let source = r#"module demo.bad_mutable_initializer

define function main returns integer
    mutable score as integer be true
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        let error =
            validate_source_file(&file).expect_err("wrong mutable initializer type should fail");
        assert!(error.contains("variable `score` declared as `integer`, got `boolean`"));
    }

    #[test]
    fn rejects_assignment_with_wrong_type() {
        let source = r#"module demo.bad_assignment

define function main returns integer
    mutable score as integer be 0
    set score to false
    return score
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("wrong assignment type should fail");
        assert!(error.contains("cannot assign `boolean` to `score` of type `integer`"));
    }

    #[test]
    fn rejects_return_without_value_in_non_void_function() {
        let source = r#"module demo.return_value

define function main returns integer
    return
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("missing return value should fail");
        assert!(error.contains("function `main` must return a value of type `integer`"));
    }

    #[test]
    fn rejects_return_value_in_void_function() {
        let source = r#"module demo.void_return

define function main returns void
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("void return value should fail");
        assert!(error.contains("function `main` cannot return a value from a void function"));
    }

    #[test]
    fn rejects_local_function_calls_with_wrong_argument_type() {
        let source = r#"module demo.bad_argument_type

define function add_one takes value as integer returns integer
    return value + 1

define function main returns integer
    return add_one(true)
"#;

        let file = parse_source(source).expect("source should parse");
        let error = validate_source_file(&file).expect_err("wrong argument type should fail");
        assert!(error.contains("function `add_one` argument 1 must be `integer`, got `boolean`"));
    }

    #[test]
    fn rejects_builtin_calls_with_wrong_argument_type() {
        let source = r#"module demo.bad_builtin_argument

import standard.io

define function main returns integer
    io.print_line(1)
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        let error =
            validate_source_file(&file).expect_err("wrong builtin argument type should fail");
        assert!(error.contains("function `io.print_line` argument 1 must be `text`, got `integer`"));
    }

    #[test]
    fn accepts_all_canonical_examples() {
        let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut seen = 0;

        for entry in std::fs::read_dir(examples_dir).expect("examples directory should exist") {
            let entry = entry.expect("directory entry should load");
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("etl") {
                continue;
            }

            let source = std::fs::read_to_string(&path).expect("example source should read");
            let file = parse_source(&source)
                .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
            validate_source_file(&file)
                .unwrap_or_else(|error| panic!("failed to typecheck {}: {error}", path.display()));
            seen += 1;
        }

        assert!(seen >= 10, "expected all canonical examples to be present");
    }
}
