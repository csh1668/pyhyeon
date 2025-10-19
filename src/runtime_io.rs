use std::collections::VecDeque;

/// Result type for read operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadResult {
    /// Successfully read a line
    Ok(String),
    /// Waiting for input (e.g., web/interactive)
    WaitingForInput,
    /// Error occurred
    Error(String),
}

/// Abstraction over runtime I/O so the VM can remain pure w.r.t. environment.
pub trait RuntimeIo {
    fn write_line(&mut self, s: &str);
    fn write(&mut self, s: &str);
    fn read_line(&mut self) -> ReadResult;
}

/// Default I/O that talks to process stdout/stdin (CLI use).
pub struct StdIo;

impl RuntimeIo for StdIo {
    fn write_line(&mut self, s: &str) {
        println!("{}", s);
    }
    fn write(&mut self, s: &str) {
        use std::io::Write;
        print!("{}", s);
        let _ = std::io::stdout().flush();
    }
    fn read_line(&mut self) -> ReadResult {
        use std::io::{self, BufRead};
        let stdin = io::stdin();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(_) => ReadResult::Ok(line.trim_end().to_string()),
            Err(e) => ReadResult::Error(e.to_string()),
        }
    }
}

/// Buffer-based I/O for browsers/tests: caller pushes input, we accumulate output.
pub struct BufferIo {
    output: String,
    input: VecDeque<String>,
}

impl BufferIo {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            input: VecDeque::new(),
        }
    }
    pub fn push_input_line<S: Into<String>>(&mut self, line: S) {
        self.input.push_back(line.into());
    }
    pub fn take_output(self) -> String {
        self.output
    }
    pub fn get_output(&self) -> &str {
        &self.output
    }
    pub fn drain_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }
    pub fn clear_output(&mut self) {
        self.output.clear();
    }
}

impl Default for BufferIo {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeIo for BufferIo {
    fn write_line(&mut self, s: &str) {
        self.output.push_str(s);
        self.output.push('\n');
    }
    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }
    fn read_line(&mut self) -> ReadResult {
        if let Some(line) = self.input.pop_front() {
            ReadResult::Ok(line)
        } else {
            ReadResult::WaitingForInput
        }
    }
}
