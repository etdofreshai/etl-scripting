#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub module_path: Vec<String>,
    pub imports: Vec<Vec<String>>,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    Record(RecordDeclaration),
    Function(FunctionDeclaration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDeclaration {
    pub name: String,
    pub fields: Vec<FieldDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDeclaration {
    pub name: String,
    pub field_type: TypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDeclaration {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeRef,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub parameter_type: TypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Let {
        name: String,
        value: String,
    },
    Mutable {
        name: String,
        value_type: TypeRef,
        value: String,
    },
    Set {
        target: String,
        value: String,
    },
    If {
        condition: String,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>,
    },
    RepeatWhile {
        condition: String,
        body: Vec<Statement>,
    },
    Return {
        value: Option<String>,
    },
    Expression {
        value: String,
    },
}
