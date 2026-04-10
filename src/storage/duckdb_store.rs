use std::path::Path;

use chrono::NaiveDateTime;
use duckdb::{params, Connection, OptionalExt};

use crate::error::{HypatiaError, Result, StorageError};
use crate::model::{Content, Knowledge, Statement, StatementKey};

const KNOWLEDGE_SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS knowledge (
    name TEXT PRIMARY KEY,
    content JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)";

const STATEMENT_SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS statement (
    triple TEXT PRIMARY KEY,
    subject TEXT,
    predicate TEXT,
    object TEXT,
    content JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    tr_start TIMESTAMP,
    tr_end TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_stmt_subject ON statement(subject);
CREATE INDEX IF NOT EXISTS idx_stmt_predicate ON statement(predicate);
CREATE INDEX IF NOT EXISTS idx_stmt_object ON statement(object)";

/// SQL fragment for selecting all knowledge columns with timestamps as strings.
const KNOWLEDGE_SELECT: &str = "\
SELECT name, content, CAST(created_at AS VARCHAR) AS created_at FROM knowledge";

/// SQL fragment for selecting all statement columns with timestamps as strings.
const STATEMENT_SELECT: &str = "\
SELECT triple, subject, predicate, object, content, \
       CAST(created_at AS VARCHAR) AS created_at, \
       CAST(tr_start AS VARCHAR) AS tr_start, \
       CAST(tr_end AS VARCHAR) AS tr_end FROM statement";

pub struct DuckDbStore {
    conn: Connection,
}

/// Convert a serde_json::Value to a DuckDB-compatible string parameter.
fn json_value_to_sql_param(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Parse a timestamp string from DuckDB into NaiveDateTime.
fn parse_timestamp(s: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
        .map_err(|e| HypatiaError::Storage(StorageError::DuckDb(
            duckdb::Error::FromSqlConversionFailure(0, duckdb::types::Type::Text, Box::new(e)),
        )))
}

/// Format a NaiveDateTime for DuckDB parameter binding.
fn format_timestamp(dt: &NaiveDateTime) -> String {
    dt.format("%Y-%m-%d %H:%M:%S%.f").to_string()
}

impl DuckDbStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(StorageError::from)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(&format!("{KNOWLEDGE_SCHEMA}; {STATEMENT_SCHEMA}"))
            .map_err(StorageError::from)?;
        Ok(())
    }

    // --- Knowledge CRUD ---

    pub fn insert_knowledge(&self, name: &str, content: &Content) -> Result<()> {
        let json = content.to_json_string();
        self.conn
            .execute(
                "INSERT INTO knowledge (name, content) VALUES (?, ?)",
                params![name, json],
            )
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub fn get_knowledge(&self, name: &str) -> Result<Option<Knowledge>> {
        let result = self
            .conn
            .query_row(
                &format!("{KNOWLEDGE_SELECT} WHERE name = ?"),
                params![name],
                |row| {
                    let name: String = row.get(0)?;
                    let json: String = row.get(1)?;
                    let created_at_str: String = row.get(2)?;
                    Ok((name, json, created_at_str))
                },
            )
            .optional()
            .map_err(StorageError::from)?;

        result
            .map(|(name, json, created_at_str)| {
                let content = Content::from_json_str(&json)?;
                let created_at = parse_timestamp(&created_at_str)?;
                Ok(Knowledge { name, content, created_at })
            })
            .transpose()
    }

    pub fn update_knowledge(&self, name: &str, content: &Content) -> Result<()> {
        let json = content.to_json_string();
        let rows = self
            .conn
            .execute(
                "UPDATE knowledge SET content = ? WHERE name = ?",
                params![json, name],
            )
            .map_err(StorageError::from)?;
        if rows == 0 {
            return Err(HypatiaError::NotFound {
                kind: "knowledge".to_string(),
                key: name.to_string(),
            });
        }
        Ok(())
    }

    pub fn delete_knowledge(&self, name: &str) -> Result<()> {
        let rows = self
            .conn
            .execute("DELETE FROM knowledge WHERE name = ?", params![name])
            .map_err(StorageError::from)?;
        if rows == 0 {
            return Err(HypatiaError::NotFound {
                kind: "knowledge".to_string(),
                key: name.to_string(),
            });
        }
        Ok(())
    }

    pub fn query_knowledge(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<Knowledge>> {
        let sql_params: Vec<String> = params.iter().map(json_value_to_sql_param).collect();
        let param_refs: Vec<&dyn duckdb::ToSql> =
            sql_params.iter().map(|s| s as &dyn duckdb::ToSql).collect();

        let mut stmt = self.conn.prepare(sql).map_err(StorageError::from)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let name: String = row.get(0)?;
                let json: String = row.get(1)?;
                let created_at_str: String = row.get(2)?;
                Ok((name, json, created_at_str))
            })
            .map_err(StorageError::from)?;

        let mut result = Vec::new();
        for row in rows {
            let (name, json, created_at_str) = row.map_err(StorageError::from)?;
            let content = Content::from_json_str(&json)?;
            let created_at = parse_timestamp(&created_at_str)?;
            result.push(Knowledge { name, content, created_at });
        }
        Ok(result)
    }

    // --- Statement CRUD ---

    pub fn insert_statement(
        &self,
        key: &StatementKey,
        content: &Content,
        tr_start: Option<NaiveDateTime>,
        tr_end: Option<NaiveDateTime>,
    ) -> Result<()> {
        let json = content.to_json_string();
        let triple = key.to_csv_key();
        let tr_start_str = tr_start.as_ref().map(format_timestamp);
        let tr_end_str = tr_end.as_ref().map(format_timestamp);
        self.conn
            .execute(
                "INSERT INTO statement (triple, subject, predicate, object, content, tr_start, tr_end) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![triple, key.subject, key.predicate, key.object, json, tr_start_str, tr_end_str],
            )
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub fn get_statement(&self, key: &StatementKey) -> Result<Option<Statement>> {
        let triple = key.to_csv_key();
        let result = self
            .conn
            .query_row(
                &format!("{STATEMENT_SELECT} WHERE triple = ?"),
                params![triple],
                |row| {
                    let triple: String = row.get(0)?;
                    let subject: String = row.get(1)?;
                    let predicate: String = row.get(2)?;
                    let object: String = row.get(3)?;
                    let json: String = row.get(4)?;
                    let created_at_str: String = row.get(5)?;
                    let tr_start_str: Option<String> = row.get(6)?;
                    let tr_end_str: Option<String> = row.get(7)?;
                    Ok((triple, subject, predicate, object, json, created_at_str, tr_start_str, tr_end_str))
                },
            )
            .optional()
            .map_err(StorageError::from)?;

        result
            .map(|(triple, subject, predicate, object, json, created_at_str, tr_start_str, tr_end_str)| {
                let content = Content::from_json_str(&json)?;
                let created_at = parse_timestamp(&created_at_str)?;
                let tr_start = tr_start_str.as_deref().map(parse_timestamp).transpose()?;
                let tr_end = tr_end_str.as_deref().map(parse_timestamp).transpose()?;
                let key = StatementKey { subject, predicate, object };
                let _ = triple; // triple is the PK, key is derived from columns
                Ok(Statement { key, content, created_at, tr_start, tr_end })
            })
            .transpose()
    }

    pub fn update_statement(
        &self,
        key: &StatementKey,
        content: &Content,
        tr_start: Option<NaiveDateTime>,
        tr_end: Option<NaiveDateTime>,
    ) -> Result<()> {
        let json = content.to_json_string();
        let triple = key.to_csv_key();
        let tr_start_str = tr_start.as_ref().map(format_timestamp);
        let tr_end_str = tr_end.as_ref().map(format_timestamp);
        let rows = self.conn.execute(
            "UPDATE statement SET content = ?, tr_start = ?, tr_end = ? WHERE triple = ?",
            params![json, tr_start_str, tr_end_str, triple],
        ).map_err(StorageError::from)?;
        if rows == 0 {
            return Err(HypatiaError::NotFound {
                kind: "statement".to_string(),
                key: triple,
            });
        }
        Ok(())
    }

    pub fn delete_statement(&self, key: &StatementKey) -> Result<()> {
        let triple = key.to_csv_key();
        let rows = self
            .conn
            .execute(
                "DELETE FROM statement WHERE triple = ?",
                params![triple],
            )
            .map_err(StorageError::from)?;
        if rows == 0 {
            return Err(HypatiaError::NotFound {
                kind: "statement".to_string(),
                key: triple,
            });
        }
        Ok(())
    }

    pub fn query_statements(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<Statement>> {
        let sql_params: Vec<String> = params.iter().map(json_value_to_sql_param).collect();
        let param_refs: Vec<&dyn duckdb::ToSql> =
            sql_params.iter().map(|s| s as &dyn duckdb::ToSql).collect();

        let mut stmt = self.conn.prepare(sql).map_err(StorageError::from)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let triple: String = row.get(0)?;
                let subject: String = row.get(1)?;
                let predicate: String = row.get(2)?;
                let object: String = row.get(3)?;
                let json: String = row.get(4)?;
                let created_at_str: String = row.get(5)?;
                let tr_start_str: Option<String> = row.get(6)?;
                let tr_end_str: Option<String> = row.get(7)?;
                Ok((triple, subject, predicate, object, json, created_at_str, tr_start_str, tr_end_str))
            })
            .map_err(StorageError::from)?;

        let mut result = Vec::new();
        for row in rows {
            let (triple, subject, predicate, object, json, created_at_str, tr_start_str, tr_end_str) =
                row.map_err(StorageError::from)?;
            let content = Content::from_json_str(&json)?;
            let created_at = parse_timestamp(&created_at_str)?;
            let tr_start = tr_start_str.as_deref().map(parse_timestamp).transpose()?;
            let tr_end = tr_end_str.as_deref().map(parse_timestamp).transpose()?;
            let key = StatementKey { subject, predicate, object };
            let _ = triple;
            result.push(Statement { key, content, created_at, tr_start, tr_end });
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Format;
    use tempfile::TempDir;

    fn setup() -> (TempDir, DuckDbStore) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.duckdb");
        let store = DuckDbStore::open(&db_path).unwrap();
        (dir, store)
    }

    #[test]
    fn schema_init() {
        let (_dir, _store) = setup();
    }

    #[test]
    fn knowledge_roundtrip() {
        let (_dir, store) = setup();
        let content = Content::new("hello world").with_tags(vec!["test".to_string()]);
        store.insert_knowledge("test-entry", &content).unwrap();

        let loaded = store.get_knowledge("test-entry").unwrap().unwrap();
        assert_eq!(loaded.name, "test-entry");
        assert_eq!(loaded.content.data, "hello world");
        assert_eq!(loaded.content.tags, vec!["test"]);
    }

    #[test]
    fn knowledge_not_found() {
        let (_dir, store) = setup();
        assert!(store.get_knowledge("nonexistent").unwrap().is_none());
    }

    #[test]
    fn knowledge_update() {
        let (_dir, store) = setup();
        store.insert_knowledge("k1", &Content::new("v1")).unwrap();

        let updated = Content::new("v2").with_format(Format::Json);
        store.update_knowledge("k1", &updated).unwrap();

        let loaded = store.get_knowledge("k1").unwrap().unwrap();
        assert_eq!(loaded.content.data, "v2");
        assert_eq!(loaded.content.format, Format::Json);
    }

    #[test]
    fn knowledge_delete() {
        let (_dir, store) = setup();
        store.insert_knowledge("k1", &Content::default()).unwrap();
        store.delete_knowledge("k1").unwrap();
        assert!(store.get_knowledge("k1").unwrap().is_none());
    }

    #[test]
    fn statement_roundtrip() {
        let (_dir, store) = setup();
        let key = StatementKey::new("Alice", "knows", "Bob");
        let content = Content::new("they are friends");
        store.insert_statement(&key, &content, None, None).unwrap();

        let loaded = store.get_statement(&key).unwrap().unwrap();
        assert_eq!(loaded.key.to_csv_key(), "Alice,knows,Bob");
        assert_eq!(loaded.content.data, "they are friends");
    }

    #[test]
    fn statement_with_temporal_range() {
        use chrono::NaiveDate;
        let (_dir, store) = setup();
        let key = StatementKey::new("Alice", "worked_at", "Company");
        let content = Content::default();
        let start = NaiveDate::from_ymd_opt(2020, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let end = NaiveDate::from_ymd_opt(2023, 12, 31)
            .unwrap()
            .and_hms_opt(23, 59, 59)
            .unwrap();
        store
            .insert_statement(&key, &content, Some(start), Some(end))
            .unwrap();

        let loaded = store.get_statement(&key).unwrap().unwrap();
        assert_eq!(loaded.tr_start, Some(start));
        assert_eq!(loaded.tr_end, Some(end));
    }

    #[test]
    fn statement_delete() {
        let (_dir, store) = setup();
        let key = StatementKey::new("A", "rel", "B");
        store
            .insert_statement(&key, &Content::default(), None, None)
            .unwrap();
        store.delete_statement(&key).unwrap();
        assert!(store.get_statement(&key).unwrap().is_none());
    }
}
