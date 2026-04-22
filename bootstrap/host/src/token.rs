#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Keyword,
    Integer,
    Float,
    Text,
    Symbol,
    Newline,
    Indent,
    Dedent,
    Eof,
}
