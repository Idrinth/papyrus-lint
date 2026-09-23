//! Unit coverage for the source renderer used by the lint crate's build
//! script generators.

#[path = "../build_support/renderer.rs"]
mod renderer;

use renderer::Renderer;

#[test]
fn renders_lines_blank_lines_and_nested_blocks() {
    let mut output = Renderer::new();
    output.line("pub fn generated() -> bool");
    output.block("mod outer", |output| {
        output.line("const ENABLED: bool = true;");
        output.blank();
        output.block("fn inner()", |output| {
            output.line("assert!(ENABLED);");
        });
    });

    assert_eq!(
        output.finish(),
        concat!(
            "pub fn generated() -> bool\n",
            "mod outer {\n",
            "    const ENABLED: bool = true;\n",
            "\n",
            "    fn inner() {\n",
            "        assert!(ENABLED);\n",
            "    }\n",
            "}\n",
        )
    );
}

#[test]
fn accepts_display_values_without_preformatting_them() {
    let mut output = Renderer::new();

    output.line(format_args!("const ANSWER: usize = {};", 42));
    output.block(format_args!("impl {}", "Generated"), |_| {});

    assert_eq!(
        output.finish(),
        "const ANSWER: usize = 42;\nimpl Generated {\n}\n"
    );
}
