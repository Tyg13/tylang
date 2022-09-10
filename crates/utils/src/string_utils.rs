pub struct ListSeparator {
    sep: String,
    first: std::cell::Cell<bool>,
}
impl Default for ListSeparator {
    fn default() -> Self {
        Self::new(", ")
    }
}

impl ListSeparator {
    pub fn new(sep: impl Into<String>) -> Self {
        Self {
            sep: sep.into(),
            first: true.into(),
        }
    }
}

impl std::fmt::Display for ListSeparator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.first.get() {
            return f.write_str(&self.sep);
        }
        self.first.set(false);
        Ok(())
    }
}
