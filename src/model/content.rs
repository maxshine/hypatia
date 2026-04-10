use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Content {
    pub format: Format,
    pub data: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Markdown,
    Json,
    Plain,
}

impl Default for Content {
    fn default() -> Self {
        Self {
            format: Format::Markdown,
            data: String::new(),
            tags: Vec::new(),
        }
    }
}

impl Content {
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            ..Self::default()
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).expect("Content serialization should not fail")
    }

    pub fn from_json_str(s: &str) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Build the text content for FTS indexing: name + serialized content.
    pub fn fts_content(&self, name: &str) -> String {
        let mut parts = vec![name.to_string(), self.data.clone()];
        for tag in &self.tags {
            parts.push(tag.clone());
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_content() {
        let c = Content::default();
        assert_eq!(c.format, Format::Markdown);
        assert!(c.data.is_empty());
        assert!(c.tags.is_empty());
    }

    #[test]
    fn serialization_roundtrip() {
        let c = Content::new("hello world")
            .with_tags(vec!["test".to_string()])
            .with_format(Format::Json);
        let json = c.to_json_string();
        let c2 = Content::from_json_str(&json).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn fts_content_includes_name_data_tags() {
        let c = Content::new("some data").with_tags(vec!["tag1".to_string(), "tag2".to_string()]);
        let fts = c.fts_content("my-knowledge");
        assert!(fts.contains("my-knowledge"));
        assert!(fts.contains("some data"));
        assert!(fts.contains("tag1"));
        assert!(fts.contains("tag2"));
    }
}
