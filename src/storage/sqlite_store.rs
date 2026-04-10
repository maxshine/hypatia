use std::path::Path;

use rusqlite::{params, OptionalExtension, Connection};

use crate::error::{Result, StorageError};
use crate::model::SearchOpts;

const DOCS_META_SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS docs_meta (
    id INTEGER PRIMARY KEY,
    catalog TEXT,
    key TEXT,
    content TEXT
)";

const DOCS_META_INDEX: &str = "\
CREATE INDEX IF NOT EXISTS idx_docs_catalog ON docs_meta(catalog)";

const DOCS_FTS_SCHEMA: &str = "\
CREATE VIRTUAL TABLE IF NOT EXISTS docs_fts USING fts5(
    content,
    content='docs_meta',
    content_rowid='id'
)";

const TRIGGER_INSERT: &str = "\
CREATE TRIGGER IF NOT EXISTS docs_ai AFTER INSERT ON docs_meta BEGIN
    INSERT INTO docs_fts(rowid, content) VALUES (new.id, new.content);
END";

const TRIGGER_DELETE: &str = "\
CREATE TRIGGER IF NOT EXISTS docs_ad AFTER DELETE ON docs_meta BEGIN
    INSERT INTO docs_fts(docs_fts, rowid, content) VALUES('delete', old.id, old.content);
END";

const TRIGGER_UPDATE: &str = "\
CREATE TRIGGER IF NOT EXISTS docs_au AFTER UPDATE ON docs_meta BEGIN
    INSERT INTO docs_fts(docs_fts, rowid, content) VALUES('delete', old.id, old.content);
    INSERT INTO docs_fts(rowid, content) VALUES (new.id, new.content);
END";

#[derive(Debug, Clone)]
pub struct FtsResult {
    pub id: i64,
    pub catalog: String,
    pub key: String,
    pub content: String,
    pub rank: f64,
}

fn row_to_fts_result(row: &rusqlite::Row) -> rusqlite::Result<FtsResult> {
    Ok(FtsResult {
        id: row.get(0)?,
        catalog: row.get(1)?,
        key: row.get(2)?,
        content: row.get(3)?,
        rank: row.get(4)?,
    })
}

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(StorageError::from)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(&format!(
                "{DOCS_META_SCHEMA}; {DOCS_META_INDEX}; {DOCS_FTS_SCHEMA}; \
                 {TRIGGER_INSERT}; {TRIGGER_DELETE}; {TRIGGER_UPDATE};"
            ))
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub fn upsert_doc(&self, catalog: &str, key: &str, content: &str) -> Result<()> {
        let existing_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM docs_meta WHERE catalog = ?1 AND key = ?2",
                params![catalog, key],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::from)?;

        match existing_id {
            Some(id) => {
                self.conn
                    .execute(
                        "UPDATE docs_meta SET content = ?1 WHERE id = ?2",
                        params![content, id],
                    )
                    .map_err(StorageError::from)?;
            }
            None => {
                self.conn
                    .execute(
                        "INSERT INTO docs_meta (catalog, key, content) VALUES (?1, ?2, ?3)",
                        params![catalog, key, content],
                    )
                    .map_err(StorageError::from)?;
            }
        }
        Ok(())
    }

    pub fn delete_doc(&self, catalog: &str, key: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM docs_meta WHERE catalog = ?1 AND key = ?2",
                params![catalog, key],
            )
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub fn search(&self, query: &str, opts: &SearchOpts) -> Result<Vec<FtsResult>> {
        let catalog_filter = opts.catalog.as_deref();
        let sql = match catalog_filter {
            Some(_) =>
                "SELECT m.id, m.catalog, m.key, m.content, f.rank \
                 FROM docs_meta m \
                 JOIN docs_fts f ON m.id = f.rowid \
                 WHERE docs_fts MATCH ?1 AND m.catalog = ?2 \
                 ORDER BY f.rank LIMIT ?3 OFFSET ?4",
            None =>
                "SELECT m.id, m.catalog, m.key, m.content, f.rank \
                 FROM docs_meta m \
                 JOIN docs_fts f ON m.id = f.rowid \
                 WHERE docs_fts MATCH ?1 \
                 ORDER BY f.rank LIMIT ?2 OFFSET ?3",
        };

        let mut stmt = self.conn.prepare(sql).map_err(StorageError::from)?;

        let rows: Vec<rusqlite::Result<FtsResult>> = match catalog_filter {
            Some(cat) => stmt
                .query_map(params![query, cat, opts.limit, opts.offset], row_to_fts_result)
                .map_err(StorageError::from)?
                .collect(),
            None => stmt
                .query_map(params![query, opts.limit, opts.offset], row_to_fts_result)
                .map_err(StorageError::from)?
                .collect(),
        };

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(StorageError::from)?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, SqliteStore) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.sqlite");
        let store = SqliteStore::open(&db_path).unwrap();
        (dir, store)
    }

    #[test]
    fn schema_init() {
        let (_dir, _store) = setup();
    }

    #[test]
    fn upsert_and_search() {
        let (_dir, store) = setup();
        store
            .upsert_doc("knowledge", "rust", "Rust is a systems programming language")
            .unwrap();
        store
            .upsert_doc("knowledge", "python", "Python is a scripting language")
            .unwrap();

        let results = store
            .search("programming", &SearchOpts::default())
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "rust");
    }

    #[test]
    fn upsert_updates_existing() {
        let (_dir, store) = setup();
        store
            .upsert_doc("knowledge", "rust", "Rust is fast")
            .unwrap();
        store
            .upsert_doc("knowledge", "rust", "Rust is fast and safe")
            .unwrap();

        let results = store.search("safe", &SearchOpts::default()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "rust");
        assert!(results[0].content.contains("safe"));
    }

    #[test]
    fn search_with_catalog_filter() {
        let (_dir, store) = setup();
        store
            .upsert_doc("knowledge", "rust", "Rust programming")
            .unwrap();
        store
            .upsert_doc("statement", "rust,is,language", "Rust is a programming language")
            .unwrap();

        let opts = SearchOpts {
            catalog: Some("knowledge".to_string()),
            ..Default::default()
        };
        let results = store.search("programming", &opts).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].catalog, "knowledge");
    }

    #[test]
    fn delete_doc() {
        let (_dir, store) = setup();
        store
            .upsert_doc("knowledge", "rust", "Rust programming language")
            .unwrap();
        store.delete_doc("knowledge", "rust").unwrap();

        let results = store.search("Rust", &SearchOpts::default()).unwrap();
        assert!(results.is_empty());
    }
}
