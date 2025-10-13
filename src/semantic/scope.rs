use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct ScopeStack {
    stack: Vec<HashSet<String>>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self {
            stack: vec![HashSet::new()],
        }
    }

    pub fn push(&mut self) {
        self.stack.push(HashSet::new());
    }

    pub fn pop(&mut self) {
        self.stack.pop();
    }

    pub fn define(&mut self, name: String) {
        if let Some(current) = self.stack.last_mut() {
            current.insert(name);
        }
    }

    pub fn is_defined(&self, name: &str) -> bool {
        for s in self.stack.iter().rev() {
            if s.contains(name) {
                return true;
            }
        }
        false
    }
}
