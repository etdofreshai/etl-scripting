use crate::ast::{Declaration, SourceFile, Statement};
use std::collections::HashSet;

const BUILTIN_TYPES: &[&str] = &["integer", "float", "boolean", "text", "void"];

pub fn validate_source_file(file: &SourceFile) -> Result<(), String> {
    let mut known_types: HashSet<String> = BUILTIN_TYPES
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    let mut seen_declarations = HashSet::new();

    for declaration in &file.declarations {
        match declaration {
            Declaration::Record(record) => {
                if !seen_declarations.insert(format!("record:{}", record.name)) {
                    return Err(format!("duplicate record declaration: {}", record.name));
                }
                known_types.insert(record.name.clone());
            }
            Declaration::Function(function) => {
                if !seen_declarations.insert(format!("function:{}", function.name)) {
                    return Err(format!("duplicate function declaration: {}", function.name));
                }
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
                for parameter in &function.parameters {
                    ensure_known_type(&known_types, &parameter.parameter_type.name)?;
                }
                ensure_known_type(&known_types, &function.return_type.name)?;
                validate_statements(&known_types, &function.body)?;
            }
        }
    }

    Ok(())
}

fn validate_statements(
    known_types: &HashSet<String>,
    statements: &[Statement],
) -> Result<(), String> {
    for statement in statements {
        match statement {
            Statement::Mutable { value_type, .. } => {
                ensure_known_type(known_types, &value_type.name)?;
            }
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                validate_statements(known_types, then_body)?;
                validate_statements(known_types, else_body)?;
            }
            Statement::RepeatWhile { body, .. } => {
                validate_statements(known_types, body)?;
            }
            Statement::Let { .. }
            | Statement::Set { .. }
            | Statement::Return { .. }
            | Statement::Expression { .. } => {}
        }
    }

    Ok(())
}

fn ensure_known_type(known_types: &HashSet<String>, type_name: &str) -> Result<(), String> {
    if known_types.contains(type_name) {
        Ok(())
    } else {
        Err(format!("unknown type: {type_name}"))
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
