use std::fmt::{self, Write};

/// A small line-oriented Rust source renderer.
pub struct Renderer {
    output: String,
    indent: usize,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    pub fn line(&mut self, line: impl fmt::Display) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{line}").expect("writing to a String cannot fail");
    }

    pub fn blank(&mut self) {
        self.output.push('\n');
    }

    pub fn block(&mut self, header: impl fmt::Display, body: impl FnOnce(&mut Self)) {
        self.line(format_args!("{header} {{"));
        self.indent += 1;
        body(self);
        self.indent -= 1;
        self.line("}");
    }

    pub fn finish(self) -> String {
        self.output
    }
}
