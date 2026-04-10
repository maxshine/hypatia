use chrono::NaiveDateTime;

use super::Content;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatementKey {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

impl StatementKey {
    pub fn new(subject: impl Into<String>, predicate: impl Into<String>, object: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }

    /// Format as CSV row for FTS key (handles commas and quotes).
    pub fn to_csv_key(&self) -> String {
        let fields = [&self.subject, &self.predicate, &self.object];
        fields
            .iter()
            .map(|f| csv_escape(f))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub key: StatementKey,
    pub content: Content,
    pub created_at: NaiveDateTime,
    pub tr_start: Option<NaiveDateTime>,
    pub tr_end: Option<NaiveDateTime>,
}

/// Escape a field for CSV: wrap in quotes if it contains comma, quote, or newline.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_key_csv_simple() {
        let key = StatementKey::new("Alice", "knows", "Bob");
        assert_eq!(key.to_csv_key(), "Alice,knows,Bob");
    }

    #[test]
    fn statement_key_csv_with_comma() {
        let key = StatementKey::new("Alice, Jr.", "knows", "Bob");
        assert_eq!(key.to_csv_key(), "\"Alice, Jr.\",knows,Bob");
    }

    #[test]
    fn statement_key_csv_with_quote() {
        let key = StatementKey::new("Alice \"Al\"", "knows", "Bob");
        assert_eq!(key.to_csv_key(), "\"Alice \"\"Al\"\"\",knows,Bob");
    }

    #[test]
    fn statement_key_equality() {
        let k1 = StatementKey::new("a", "b", "c");
        let k2 = StatementKey::new("a", "b", "c");
        let k3 = StatementKey::new("a", "b", "d");
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }
}
