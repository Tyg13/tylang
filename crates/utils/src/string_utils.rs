pub struct ListSeparator {
    sep: String,
    first: std::cell::Cell<bool>,
}

impl ListSeparator {
    pub fn new(sep: impl Into<String>) -> Self {
        Self {
            sep: sep.into(),
            first: true.into(),
        }
    }

    pub fn space() -> Self {
        Self::new(" ")
    }

    pub fn nl() -> Self {
        Self::new("\n")
    }

    pub fn comma_space() -> Self {
        Self::new(", ")
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

/// Trim off leading and trailing quotation marks '"', and handle escape
/// sequences (e.g. '\n')
/// ```
/// assert_eq!(utils::trim_and_unescape(r#""foo""#), "foo");
/// assert_eq!(utils::trim_and_unescape(r#""bar\n""#), "bar\n");
/// assert_eq!(utils::trim_and_unescape(r#""baz\nbip\rbing""#), "baz\nbip\rbing");
/// assert_eq!(utils::trim_and_unescape(r#""baz\\\rbip""#), "baz\\\rbip");
/// assert_eq!(utils::trim_and_unescape(r#""baz\\\dbip""#), r"baz\\dbip");
/// assert_eq!(utils::trim_and_unescape(r#""""#), "");
/// ```
pub fn trim_and_unescape(s: &str) -> String {
    debug_assert!(s.len() >= 2);
    debug_assert!(s.chars().next() == Some('"'));
    debug_assert!(s.chars().last() == Some('"'));

    let mut res = String::with_capacity(s.len());
    let mut in_escape = false;
    for c in s[1..s.len() - 1].chars() {
        if !in_escape && c == '\\' {
            in_escape = true;
            continue;
        }
        match c {
            'n' if in_escape => res.push('\n'),
            'r' if in_escape => res.push('\r'),
            '\\' if in_escape => res.push('\\'),
            _ if in_escape => {
                res.push('\\');
                res.push(c);
            }
            _ => {
                res.push(c);
            }
        }
        in_escape = false;
    }
    res
}
