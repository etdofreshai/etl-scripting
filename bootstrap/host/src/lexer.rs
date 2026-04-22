use crate::token::{Token, TokenKind};

const KEYWORDS: &[&str] = &[
    "module", "import", "define", "record", "function", "takes", "returns", "let", "mutable", "be",
    "set", "to", "if", "else", "repeat", "while", "return", "as", "true", "false", "and", "or",
    "not",
];

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize();
    lexer.tokens
}

struct Lexer<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    indent_stack: Vec<usize>,
    pending_indent_check: bool,
    line: usize,
    column: usize,
    index: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            tokens: Vec::new(),
            indent_stack: vec![0],
            pending_indent_check: true,
            line: 1,
            column: 1,
            index: 0,
        }
    }

    fn tokenize(&mut self) {
        while let Some(ch) = self.peek_char() {
            if self.pending_indent_check {
                if ch == '\n' {
                    self.consume_newline();
                    continue;
                }
                if ch == '#' {
                    self.consume_comment();
                    continue;
                }
                self.consume_indentation();
                self.pending_indent_check = false;
                if self.peek_char().is_none() {
                    break;
                }
                if matches!(self.peek_char(), Some('\n')) {
                    continue;
                }
                if matches!(self.peek_char(), Some('#')) {
                    self.consume_comment();
                    continue;
                }
            }

            let ch = match self.peek_char() {
                Some(ch) => ch,
                None => break,
            };

            match ch {
                ' ' | '\t' => {
                    self.advance_char();
                }
                '#' => self.consume_comment(),
                '\n' => self.consume_newline(),
                'a'..='z' | 'A'..='Z' | '_' => self.consume_identifier_or_keyword(),
                '0'..='9' => self.consume_number(),
                '"' => self.consume_text(),
                '.' => self.push_simple(TokenKind::Dot, "."),
                ',' => self.push_simple(TokenKind::Comma, ","),
                '(' => self.push_simple(TokenKind::LeftParen, "("),
                ')' => self.push_simple(TokenKind::RightParen, ")"),
                '+' => self.push_simple(TokenKind::Plus, "+"),
                '-' => self.push_simple(TokenKind::Minus, "-"),
                '*' => self.push_simple(TokenKind::Star, "*"),
                '/' => self.push_simple(TokenKind::Slash, "/"),
                '<' => self.consume_less(),
                '>' => self.consume_greater(),
                '=' => self.consume_equals(),
                _ => {
                    self.advance_char();
                }
            }
        }

        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.tokens.push(Token::new(
                TokenKind::Dedent,
                "<dedent>",
                self.line,
                self.column,
            ));
        }
        self.tokens
            .push(Token::new(TokenKind::Eof, "<eof>", self.line, self.column));
    }

    fn consume_indentation(&mut self) {
        let mut spaces = 0;
        let line = self.line;
        let column = self.column;

        while matches!(self.peek_char(), Some(' ')) {
            spaces += 1;
            self.advance_char();
        }

        let current = *self.indent_stack.last().unwrap_or(&0);
        if matches!(self.peek_char(), Some('\n') | Some('#') | None) {
            return;
        }

        if spaces > current {
            self.indent_stack.push(spaces);
            self.tokens
                .push(Token::new(TokenKind::Indent, "<indent>", line, column));
        } else if spaces < current {
            while let Some(&top) = self.indent_stack.last() {
                if spaces < top {
                    self.indent_stack.pop();
                    self.tokens
                        .push(Token::new(TokenKind::Dedent, "<dedent>", line, column));
                } else {
                    break;
                }
            }
        }
    }

    fn consume_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == '\n' {
                break;
            }
            self.advance_char();
        }
    }

    fn consume_newline(&mut self) {
        let line = self.line;
        let column = self.column;
        self.advance_char();
        self.tokens
            .push(Token::new(TokenKind::Newline, "\\n", line, column));
        self.pending_indent_check = true;
    }

    fn consume_identifier_or_keyword(&mut self) {
        let line = self.line;
        let column = self.column;
        let mut lexeme = String::new();

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                lexeme.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }

        let kind = if KEYWORDS.contains(&lexeme.as_str()) {
            TokenKind::Keyword
        } else {
            TokenKind::Identifier
        };

        self.tokens.push(Token::new(kind, lexeme, line, column));
    }

    fn consume_number(&mut self) {
        let line = self.line;
        let column = self.column;
        let mut lexeme = String::new();
        let mut seen_dot = false;

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                lexeme.push(ch);
                self.advance_char();
            } else if ch == '.' && !seen_dot {
                seen_dot = true;
                lexeme.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }

        let kind = if seen_dot {
            TokenKind::Float
        } else {
            TokenKind::Integer
        };
        self.tokens.push(Token::new(kind, lexeme, line, column));
    }

    fn consume_text(&mut self) {
        let line = self.line;
        let column = self.column;
        let mut lexeme = String::new();
        self.advance_char();

        while let Some(ch) = self.peek_char() {
            if ch == '"' {
                self.advance_char();
                break;
            }
            if ch == '\\' {
                lexeme.push(ch);
                self.advance_char();
                if let Some(escaped) = self.peek_char() {
                    lexeme.push(escaped);
                    self.advance_char();
                }
                continue;
            }
            lexeme.push(ch);
            self.advance_char();
        }

        self.tokens
            .push(Token::new(TokenKind::Text, lexeme, line, column));
    }

    fn consume_less(&mut self) {
        let line = self.line;
        let column = self.column;
        self.advance_char();
        if matches!(self.peek_char(), Some('=')) {
            self.advance_char();
            self.tokens
                .push(Token::new(TokenKind::LessEqual, "<=", line, column));
        } else {
            self.tokens
                .push(Token::new(TokenKind::Less, "<", line, column));
        }
    }

    fn consume_greater(&mut self) {
        let line = self.line;
        let column = self.column;
        self.advance_char();
        if matches!(self.peek_char(), Some('=')) {
            self.advance_char();
            self.tokens
                .push(Token::new(TokenKind::GreaterEqual, ">=", line, column));
        } else {
            self.tokens
                .push(Token::new(TokenKind::Greater, ">", line, column));
        }
    }

    fn consume_equals(&mut self) {
        let line = self.line;
        let column = self.column;
        self.advance_char();
        if matches!(self.peek_char(), Some('=')) {
            self.advance_char();
            self.tokens
                .push(Token::new(TokenKind::EqualEqual, "==", line, column));
        }
    }

    fn push_simple(&mut self, kind: TokenKind, lexeme: &str) {
        let line = self.line;
        let column = self.column;
        self.advance_char();
        self.tokens.push(Token::new(kind, lexeme, line, column));
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.index..].chars().next()
    }

    fn advance_char(&mut self) {
        if let Some(ch) = self.peek_char() {
            self.index += ch.len_utf8();
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tokenize;
    use crate::token::{Token, TokenKind};

    fn token(kind: TokenKind, lexeme: &str) -> Token {
        Token::new(kind, lexeme, 1, 1)
    }

    #[test]
    fn tokenizes_module_header_and_basic_statements() {
        let source = r#"module demo.hello_world

import standard.io

define function main returns integer
    return 0
"#;

        let tokens = tokenize(source);

        assert_eq!(
            tokens,
            vec![
                token(TokenKind::Keyword, "module"),
                token(TokenKind::Identifier, "demo"),
                token(TokenKind::Dot, "."),
                token(TokenKind::Identifier, "hello_world"),
                token(TokenKind::Newline, "\\n"),
                token(TokenKind::Newline, "\\n"),
                token(TokenKind::Keyword, "import"),
                token(TokenKind::Identifier, "standard"),
                token(TokenKind::Dot, "."),
                token(TokenKind::Identifier, "io"),
                token(TokenKind::Newline, "\\n"),
                token(TokenKind::Newline, "\\n"),
                token(TokenKind::Keyword, "define"),
                token(TokenKind::Keyword, "function"),
                token(TokenKind::Identifier, "main"),
                token(TokenKind::Keyword, "returns"),
                token(TokenKind::Identifier, "integer"),
                token(TokenKind::Newline, "\\n"),
                token(TokenKind::Indent, "<indent>"),
                token(TokenKind::Keyword, "return"),
                token(TokenKind::Integer, "0"),
                token(TokenKind::Newline, "\\n"),
                token(TokenKind::Dedent, "<dedent>"),
                token(TokenKind::Eof, "<eof>"),
            ]
        );
    }

    #[test]
    fn skips_comments_and_preserves_indentation_structure() {
        let source = r#"define function test returns integer
    # comment line
    if true
        return 1
    return 0
"#;

        let tokens = tokenize(source);
        let kinds: Vec<TokenKind> = tokens.iter().map(|token| token.kind.clone()).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword,
                TokenKind::Keyword,
                TokenKind::Identifier,
                TokenKind::Keyword,
                TokenKind::Identifier,
                TokenKind::Newline,
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Keyword,
                TokenKind::Keyword,
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Keyword,
                TokenKind::Integer,
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Keyword,
                TokenKind::Integer,
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Eof,
            ]
        );
    }
}
