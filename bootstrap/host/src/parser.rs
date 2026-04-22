use crate::ast::{
    Declaration, FieldDeclaration, FunctionDeclaration, Parameter, RecordDeclaration, SourceFile,
    Statement, TypeRef,
};
use crate::lexer::tokenize;
use crate::token::{Token, TokenKind};

pub fn parse_source(source: &str) -> Result<SourceFile, String> {
    let tokens = tokenize(source);
    Parser::new(tokens).parse_source_file()
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn parse_source_file(&mut self) -> Result<SourceFile, String> {
        self.skip_newlines();
        self.expect_keyword("module")?;
        let module_path = self.parse_path()?;
        self.expect_newline()?;
        self.skip_newlines();

        let mut imports = Vec::new();
        while self.check_keyword("import") {
            self.expect_keyword("import")?;
            imports.push(self.parse_path()?);
            self.expect_newline()?;
            self.skip_newlines();
        }

        let mut declarations = Vec::new();
        while !self.is_at_end() {
            if self.check_kind(TokenKind::Eof) {
                break;
            }
            if self.check_kind(TokenKind::Newline) {
                self.advance();
                continue;
            }

            declarations.push(self.parse_declaration()?);
            self.skip_newlines();
        }

        Ok(SourceFile {
            module_path,
            imports,
            declarations,
        })
    }

    fn parse_declaration(&mut self) -> Result<Declaration, String> {
        self.expect_keyword("define")?;
        if self.check_keyword("record") {
            self.expect_keyword("record")?;
            Ok(Declaration::Record(self.parse_record_declaration()?))
        } else if self.check_keyword("function") {
            self.expect_keyword("function")?;
            Ok(Declaration::Function(self.parse_function_declaration()?))
        } else {
            Err(self.error("expected record or function declaration"))
        }
    }

    fn parse_record_declaration(&mut self) -> Result<RecordDeclaration, String> {
        let name = self.expect_identifier()?;
        self.expect_newline()?;
        self.expect_kind(TokenKind::Indent)?;

        let mut fields = Vec::new();
        while !self.check_kind(TokenKind::Dedent) && !self.is_at_end() {
            if self.check_kind(TokenKind::Newline) {
                self.advance();
                continue;
            }
            let field_name = self.expect_identifier()?;
            self.expect_keyword("as")?;
            let field_type = self.parse_type_ref()?;
            self.expect_newline()?;
            fields.push(FieldDeclaration {
                name: field_name,
                field_type,
            });
        }

        self.expect_kind(TokenKind::Dedent)?;
        Ok(RecordDeclaration { name, fields })
    }

    fn parse_function_declaration(&mut self) -> Result<FunctionDeclaration, String> {
        let name = self.expect_identifier()?;
        let mut parameters = Vec::new();

        if self.check_keyword("takes") {
            self.expect_keyword("takes")?;
            loop {
                let parameter_name = self.expect_identifier()?;
                self.expect_keyword("as")?;
                let parameter_type = self.parse_type_ref()?;
                parameters.push(Parameter {
                    name: parameter_name,
                    parameter_type,
                });

                if self.check_kind(TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.expect_keyword("returns")?;
        let return_type = self.parse_type_ref()?;
        self.expect_newline()?;
        self.expect_kind(TokenKind::Indent)?;
        let body = self.parse_block()?;

        Ok(FunctionDeclaration {
            name,
            parameters,
            return_type,
            body,
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, String> {
        let mut statements = Vec::new();
        while !self.check_kind(TokenKind::Dedent) && !self.is_at_end() {
            if self.check_kind(TokenKind::Newline) {
                self.advance();
                continue;
            }
            statements.push(self.parse_statement()?);
        }
        self.expect_kind(TokenKind::Dedent)?;
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        if self.check_keyword("let") {
            self.expect_keyword("let")?;
            let name = self.expect_identifier()?;
            self.expect_keyword("be")?;
            let value = self.collect_until_newline();
            self.expect_newline()?;
            return Ok(Statement::Let { name, value });
        }

        if self.check_keyword("mutable") {
            self.expect_keyword("mutable")?;
            let name = self.expect_identifier()?;
            self.expect_keyword("as")?;
            let value_type = self.parse_type_ref()?;
            self.expect_keyword("be")?;
            let value = self.collect_until_newline();
            self.expect_newline()?;
            return Ok(Statement::Mutable {
                name,
                value_type,
                value,
            });
        }

        if self.check_keyword("set") {
            self.expect_keyword("set")?;
            let target = self.collect_until_keyword("to");
            self.expect_keyword("to")?;
            let value = self.collect_until_newline();
            self.expect_newline()?;
            return Ok(Statement::Set { target, value });
        }

        if self.check_keyword("if") {
            self.expect_keyword("if")?;
            let condition = self.collect_until_newline();
            self.expect_newline()?;
            self.expect_kind(TokenKind::Indent)?;
            let then_body = self.parse_block()?;
            self.skip_newlines();

            let else_body = if self.check_keyword("else") {
                self.expect_keyword("else")?;
                self.expect_newline()?;
                self.expect_kind(TokenKind::Indent)?;
                self.parse_block()?
            } else {
                Vec::new()
            };

            return Ok(Statement::If {
                condition,
                then_body,
                else_body,
            });
        }

        if self.check_keyword("repeat") {
            self.expect_keyword("repeat")?;
            self.expect_keyword("while")?;
            let condition = self.collect_until_newline();
            self.expect_newline()?;
            self.expect_kind(TokenKind::Indent)?;
            let body = self.parse_block()?;
            return Ok(Statement::RepeatWhile { condition, body });
        }

        if self.check_keyword("return") {
            self.expect_keyword("return")?;
            let value = if self.check_kind(TokenKind::Newline) {
                None
            } else {
                Some(self.collect_until_newline())
            };
            self.expect_newline()?;
            return Ok(Statement::Return { value });
        }

        let value = self.collect_until_newline();
        self.expect_newline()?;
        Ok(Statement::Expression { value })
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, String> {
        Ok(TypeRef {
            name: self.parse_path()?.join("."),
        })
    }

    fn parse_path(&mut self) -> Result<Vec<String>, String> {
        let mut path = vec![self.expect_identifier()?];
        while self.check_kind(TokenKind::Dot) {
            self.advance();
            path.push(self.expect_identifier()?);
        }
        Ok(path)
    }

    fn collect_until_keyword(&mut self, keyword: &str) -> String {
        let start = self.index;
        while !self.is_at_end() && !self.check_keyword(keyword) {
            if self.check_kind(TokenKind::Newline) {
                break;
            }
            self.advance();
        }
        self.tokens_to_source(start, self.index)
    }

    fn collect_until_newline(&mut self) -> String {
        let start = self.index;
        while !self.is_at_end() && !self.check_kind(TokenKind::Newline) {
            self.advance();
        }
        self.tokens_to_source(start, self.index)
    }

    fn tokens_to_source(&self, start: usize, end: usize) -> String {
        let mut result = String::new();
        for token in &self.tokens[start..end] {
            match token.kind {
                TokenKind::Dot => result.push('.'),
                TokenKind::Comma => {
                    result.push(',');
                    result.push(' ');
                }
                TokenKind::LeftParen => result.push('('),
                TokenKind::RightParen => result.push(')'),
                TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::EqualEqual
                | TokenKind::Less
                | TokenKind::LessEqual
                | TokenKind::Greater
                | TokenKind::GreaterEqual => {
                    if !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.push_str(&token.lexeme);
                    result.push(' ');
                }
                _ => {
                    if !result.is_empty() && !result.ends_with([' ', '(', '.']) {
                        result.push(' ');
                    }
                    if token.kind == TokenKind::Text {
                        result.push('"');
                        result.push_str(&token.lexeme);
                        result.push('"');
                    } else {
                        result.push_str(&token.lexeme);
                    }
                }
            }
        }
        result.trim().to_string()
    }

    fn skip_newlines(&mut self) {
        while self.check_kind(TokenKind::Newline) {
            self.advance();
        }
    }

    fn expect_identifier(&mut self) -> Result<String, String> {
        let token = self.expect_kind(TokenKind::Identifier)?;
        Ok(token.lexeme)
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), String> {
        let token = self.peek();
        if token.kind == TokenKind::Keyword && token.lexeme == keyword {
            self.advance();
            Ok(())
        } else {
            Err(self.error(&format!("expected keyword `{keyword}`")))
        }
    }

    fn expect_newline(&mut self) -> Result<(), String> {
        self.expect_kind(TokenKind::Newline).map(|_| ())
    }

    fn expect_kind(&mut self, kind: TokenKind) -> Result<Token, String> {
        let token = self.peek().clone();
        if token.kind == kind {
            self.advance();
            Ok(token)
        } else {
            Err(self.error(&format!("expected {:?}", kind)))
        }
    }

    fn check_keyword(&self, keyword: &str) -> bool {
        let token = self.peek();
        token.kind == TokenKind::Keyword && token.lexeme == keyword
    }

    fn check_kind(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.index += 1;
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.index.min(self.tokens.len() - 1)]
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn error(&self, message: &str) -> String {
        let token = self.peek();
        format!(
            "parse error at line {}, column {}: {}",
            token.span.line, token.span.column, message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::parse_source;
    use crate::ast::{Declaration, Statement};

    #[test]
    fn parses_module_imports_and_main_function() {
        let source = r#"module demo.hello_world

import standard.io

define function main returns integer
    io.print_line("Hello from ETL")
    return 0
"#;

        let file = parse_source(source).expect("source should parse");

        assert_eq!(file.module_path, vec!["demo", "hello_world"]);
        assert_eq!(file.imports, vec![vec!["standard", "io"]]);
        assert_eq!(file.declarations.len(), 1);

        match &file.declarations[0] {
            Declaration::Function(function) => {
                assert_eq!(function.name, "main");
                assert!(function.parameters.is_empty());
                assert_eq!(function.return_type.name, "integer");
                assert_eq!(function.body.len(), 2);
                assert!(matches!(&function.body[0], Statement::Expression { .. }));
                assert!(matches!(&function.body[1], Statement::Return { .. }));
            }
            other => panic!("expected function declaration, got {other:?}"),
        }
    }

    #[test]
    fn parses_record_and_function_with_nested_control_flow() {
        let source = r#"module game.entity_state

define record entity_state
    id as integer
    health as integer
    stamina as integer
    active as boolean


define function can_act takes entity as entity_state returns boolean
    if entity.active == false
        return false

    return entity.health > 0 and entity.stamina > 0
"#;

        let file = parse_source(source).expect("source should parse");

        assert_eq!(file.declarations.len(), 2);

        match &file.declarations[0] {
            Declaration::Record(record) => {
                assert_eq!(record.name, "entity_state");
                assert_eq!(record.fields.len(), 4);
                assert_eq!(record.fields[0].name, "id");
                assert_eq!(record.fields[0].field_type.name, "integer");
            }
            other => panic!("expected record declaration, got {other:?}"),
        }

        match &file.declarations[1] {
            Declaration::Function(function) => {
                assert_eq!(function.name, "can_act");
                assert_eq!(function.parameters.len(), 1);
                assert_eq!(function.parameters[0].name, "entity");
                assert_eq!(function.body.len(), 2);
                assert!(matches!(&function.body[0], Statement::If { .. }));
                assert!(matches!(&function.body[1], Statement::Return { .. }));
            }
            other => panic!("expected function declaration, got {other:?}"),
        }
    }

    #[test]
    fn parses_all_canonical_examples() {
        let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut seen = 0;

        for entry in std::fs::read_dir(examples_dir).expect("examples directory should exist") {
            let entry = entry.expect("directory entry should load");
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("etl") {
                continue;
            }

            let source = std::fs::read_to_string(&path).expect("example source should read");
            parse_source(&source)
                .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
            seen += 1;
        }

        assert!(seen >= 10, "expected all canonical examples to be present");
    }
}
