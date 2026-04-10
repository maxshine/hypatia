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

    /// Format as CSV row for FTS key / triple column (handles commas and quotes).
    pub fn to_csv_key(&self) -> String {
        let fields = [&self.subject, &self.predicate, &self.object];
        fields
            .iter()
            .map(|f| csv_escape(f))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Parse a CSV-formatted triple back into a StatementKey.
    pub fn from_csv(csv: &str) -> Option<Self> {
        let fields = csv_split(csv);
        if fields.len() == 3 {
            Some(Self {
                subject: fields[0].clone(),
                predicate: fields[1].clone(),
                object: fields[2].clone(),
            })
        } else {
            None
        }
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

/// Split a CSV line respecting quoted fields.
pub fn csv_split(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            result.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    result.push(current);
    result
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

    #[test]
    fn from_csv_roundtrip() {
        let key = StatementKey::new("Alice", "knows", "Bob");
        let csv = key.to_csv_key();
        let parsed = StatementKey::from_csv(&csv).unwrap();
        assert_eq!(parsed.subject, "Alice");
        assert_eq!(parsed.predicate, "knows");
        assert_eq!(parsed.object, "Bob");
    }

    #[test]
    fn from_csv_with_comma() {
        let key = StatementKey::new("Alice, Jr.", "knows", "Bob");
        let csv = key.to_csv_key();
        let parsed = StatementKey::from_csv(&csv).unwrap();
        assert_eq!(parsed.subject, "Alice, Jr.");
    }

    #[test]
    fn from_csv_invalid() {
        assert!(StatementKey::from_csv("only,two").is_none());
    }

    #[test]
    fn csv_split_simple() {
        assert_eq!(csv_split("Alice,knows,Bob"), vec!["Alice", "knows", "Bob"]);
    }

    #[test]
    fn csv_split_quoted() {
        assert_eq!(csv_split("\"Alice, Jr.\",knows,Bob"), vec!["Alice, Jr.", "knows", "Bob"]);
    }
}
