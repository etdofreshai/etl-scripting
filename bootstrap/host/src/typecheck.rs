use crate::ast::{Declaration, FunctionDeclaration, SourceFile, Statement};
use std::collections::{HashMap, HashSet};

const BUILTIN_TYPES: &[&str] = &["integer", "float", "boolean", "text", "void"];

struct FunctionSignature {
    parameter_count: usize,
}

pub fn validate_source_file(file: &SourceFile) -> Result<(), String> {
    let mut known_types: HashSet<String> = BUILTIN_TYPES
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    let mut record_fields: HashMap<String, HashSet<String>> = HashMap::new();
    let mut functions = HashMap::new();
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
                        .map(|field| field.name.clone())
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
                        parameter_count: function.parameters.len(),
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
    record_fields: &HashMap<String, HashSet<String>>,
    functions: &HashMap<String, FunctionSignature>,
) -> Result<(), String> {
    let mut scope = HashSet::new();
    for parameter in &function.parameters {
        ensure_known_type(known_types, &parameter.parameter_type.name)?;
        scope.insert(parameter.name.clone());
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
    record_fields: &HashMap<String, HashSet<String>>,
    functions: &HashMap<String, FunctionSignature>,
    function_name: &str,
    function_return_type: &str,
    scope: &mut HashSet<String>,
    statements: &[Statement],
) -> Result<(), String> {
    for statement in statements {
        match statement {
            Statement::Let { name, value } => {
                validate_expression(value, scope, known_types, record_fields, functions)?;
                scope.insert(name.clone());
            }
            Statement::Mutable {
                name,
                value_type,
                value,
            } => {
                ensure_known_type(known_types, &value_type.name)?;
                validate_expression(value, scope, known_types, record_fields, functions)?;
                scope.insert(name.clone());
            }
            Statement::Set { target, value } => {
                validate_reference(target, scope)?;
                validate_expression(value, scope, known_types, record_fields, functions)?;
            }
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                validate_expression(condition, scope, known_types, record_fields, functions)?;
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
                validate_expression(condition, scope, known_types, record_fields, functions)?;
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
    scope: &HashSet<String>,
    known_types: &HashSet<String>,
    record_fields: &HashMap<String, HashSet<String>>,
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
            validate_expression(expression, scope, known_types, record_fields, functions)
        }
        (_, None) => Ok(()),
    }
}

fn validate_expression(
    expression: &str,
    scope: &HashSet<String>,
    known_types: &HashSet<String>,
    record_fields: &HashMap<String, HashSet<String>>,
    functions: &HashMap<String, FunctionSignature>,
) -> Result<(), String> {
    let tokens = ExprLexer::new(expression).tokenize()?;
    let mut parser = ExprParser::new(tokens, scope, known_types, record_fields, functions);
    parser.parse_expression()?;
    parser.ensure_complete()
}

fn validate_reference(reference: &str, scope: &HashSet<String>) -> Result<(), String> {
    let base = reference.split('.').next().unwrap_or(reference);
    if scope.contains(base) {
        Ok(())
    } else {
        Err(format!("unknown variable: {base}"))
    }
}

fn ensure_known_type(known_types: &HashSet<String>, type_name: &str) -> Result<(), String> {
    if known_types.contains(type_name) {
        Ok(())
    } else {
        Err(format!("unknown type: {type_name}"))
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

struct ExprParser<'a> {
    tokens: Vec<ExprToken>,
    index: usize,
    scope: &'a HashSet<String>,
    known_types: &'a HashSet<String>,
    record_fields: &'a HashMap<String, HashSet<String>>,
    functions: &'a HashMap<String, FunctionSignature>,
}

impl<'a> ExprParser<'a> {
    fn new(
        tokens: Vec<ExprToken>,
        scope: &'a HashSet<String>,
        known_types: &'a HashSet<String>,
        record_fields: &'a HashMap<String, HashSet<String>>,
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

    fn parse_expression(&mut self) -> Result<(), String> {
        self.parse_or()
    }

    fn ensure_complete(&self) -> Result<(), String> {
        if let Some(token) = self.tokens.get(self.index) {
            Err(format!("unexpected expression token: {token:?}"))
        } else {
            Ok(())
        }
    }

    fn parse_or(&mut self) -> Result<(), String> {
        self.parse_and()?;
        while self.match_token(&ExprToken::Or) {
            self.parse_and()?;
        }
        Ok(())
    }

    fn parse_and(&mut self) -> Result<(), String> {
        self.parse_comparison()?;
        while self.match_token(&ExprToken::And) {
            self.parse_comparison()?;
        }
        Ok(())
    }

    fn parse_comparison(&mut self) -> Result<(), String> {
        self.parse_term()?;
        loop {
            if self.match_token(&ExprToken::EqualEqual)
                || self.match_token(&ExprToken::Less)
                || self.match_token(&ExprToken::Greater)
                || self.match_token(&ExprToken::LessEqual)
                || self.match_token(&ExprToken::GreaterEqual)
            {
                self.parse_term()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_term(&mut self) -> Result<(), String> {
        self.parse_factor()?;
        loop {
            if self.match_token(&ExprToken::Plus) || self.match_token(&ExprToken::Minus) {
                self.parse_factor()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_factor(&mut self) -> Result<(), String> {
        self.parse_primary()?;
        loop {
            if self.match_token(&ExprToken::Star) || self.match_token(&ExprToken::Slash) {
                self.parse_primary()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_primary(&mut self) -> Result<(), String> {
        match self.advance().cloned() {
            Some(ExprToken::Integer(_))
            | Some(ExprToken::Text(_))
            | Some(ExprToken::True)
            | Some(ExprToken::False) => Ok(()),
            Some(ExprToken::Identifier(name)) => self.parse_identifier_expression(name),
            Some(ExprToken::LeftParen) => {
                self.parse_expression()?;
                self.expect(&ExprToken::RightParen)
            }
            other => Err(format!("unexpected expression token: {other:?}")),
        }
    }

    fn parse_identifier_expression(&mut self, first: String) -> Result<(), String> {
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

        validate_reference(&name, self.scope)
    }

    fn parse_call(&mut self, name: &str) -> Result<(), String> {
        let argument_count = self.parse_call_arguments()?;
        if name.contains('.') {
            return Ok(());
        }

        let signature = self
            .functions
            .get(name)
            .ok_or_else(|| format!("unknown function: {name}"))?;
        if signature.parameter_count != argument_count {
            return Err(format!(
                "function `{name}` expects {} arguments, got {argument_count}",
                signature.parameter_count
            ));
        }
        Ok(())
    }

    fn parse_record_constructor(&mut self, type_name: &str) -> Result<(), String> {
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
            if !known_fields.contains(&field_name) {
                return Err(format!(
                    "unknown field `{field_name}` for record `{type_name}`"
                ));
            }
            self.parse_expression()?;
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
        known_fields: &HashSet<String>,
        seen_fields: HashSet<String>,
    ) -> Result<(), String> {
        for field_name in known_fields {
            if !seen_fields.contains(field_name) {
                return Err(format!(
                    "missing field `{field_name}` for record `{type_name}`"
                ));
            }
        }
        Ok(())
    }

    fn parse_call_arguments(&mut self) -> Result<usize, String> {
        if self.match_token(&ExprToken::RightParen) {
            return Ok(0);
        }

        let mut count = 0;
        loop {
            self.parse_expression()?;
            count += 1;
            if self.match_token(&ExprToken::Comma) {
                continue;
            }
            self.expect(&ExprToken::RightParen)?;
            break;
        }
        Ok(count)
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
