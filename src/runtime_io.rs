use std::collections::VecDeque;

/// Abstraction over runtime I/O so the VM can remain pure w.r.t. environment.
pub trait RuntimeIo {
    fn write_line(&mut self, s: &str);
    fn read_line(&mut self) -> Result<String, String>;
}

/// Default I/O that talks to process stdout/stdin (CLI use).
pub struct StdIo;

impl RuntimeIo for StdIo {
    fn write_line(&mut self, s: &str) { println!("{}", s); }
    fn read_line(&mut self) -> Result<String, String> {
        use std::io::{self, Read};
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).map_err(|e| e.to_string())?;
        Ok(buf.lines().next().unwrap_or("").to_string())
    }
}

/// Buffer-based I/O for browsers/tests: caller pushes input, we accumulate output.
pub struct BufferIo {
    output: String,
    input: VecDeque<String>,
}

impl BufferIo {
    pub fn new() -> Self { Self { output: String::new(), input: VecDeque::new() } }
    pub fn push_input_line<S: Into<String>>(&mut self, line: S) { self.input.push_back(line.into()); }
    pub fn take_output(self) -> String { self.output }
}

impl Default for BufferIo { fn default() -> Self { Self::new() } }

impl RuntimeIo for BufferIo {
    fn write_line(&mut self, s: &str) { self.output.push_str(s); self.output.push('\n'); }
    fn read_line(&mut self) -> Result<String, String> {
        if let Some(line) = self.input.pop_front() { Ok(line) } else { Err("no input available".into()) }
    }
}


