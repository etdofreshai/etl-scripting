use crate::asm;
use crate::lir::LinearProgram;

pub fn render_program(program: &LinearProgram, target: &str) -> Result<String, String> {
    match target {
        "linux-x86_64" => Ok(render_linux_x86_64(program)),
        other => Err(format!("unsupported native target: {other}")),
    }
}

fn render_linux_x86_64(program: &LinearProgram) -> String {
    let mut output = String::new();
    output.push_str("target linux-x86_64\n");
    output.push_str("format elf64\n\n");
    output.push_str(&asm::render_program(program));
    output
}

#[cfg(test)]
mod tests {
    use super::render_program;
    use crate::ir::lower_source_file;
    use crate::lir::lower_program;
    use crate::parser::parse_source;
    use crate::typecheck::validate_source_file;

    #[test]
    fn renders_linux_x86_64_native_output() {
        let source = r#"module demo.native

define function main returns integer
    return 0
"#;

        let file = parse_source(source).expect("source should parse");
        validate_source_file(&file).expect("source should validate");
        let ir = lower_source_file(&file);
        let linear = lower_program(&ir).expect("linear lowering should succeed");
        let native =
            render_program(&linear, "linux-x86_64").expect("native rendering should succeed");

        assert!(native.contains("target linux-x86_64"));
        assert!(native.contains("format elf64"));
        assert!(native.contains("section .text"));
        assert!(native.contains("global main"));
    }
}
