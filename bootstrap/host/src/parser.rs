use crate::ast::Module;

pub fn parse_module(_source: &str) -> Result<Module, String> {
    Ok(Module { name: "placeholder".to_string() })
}
