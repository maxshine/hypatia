use chrono::NaiveDateTime;

use crate::error::Result;
use crate::model::{Content, Statement, StatementKey};
use crate::storage::OpenShelf;

pub struct StatementService<'a> {
    shelf: &'a mut OpenShelf,
}

impl<'a> StatementService<'a> {
    pub fn new(shelf: &'a mut OpenShelf) -> Self {
        Self { shelf }
    }

    pub fn create(
        &mut self,
        key: &StatementKey,
        content: Content,
        tr_start: Option<NaiveDateTime>,
        tr_end: Option<NaiveDateTime>,
    ) -> Result<Statement> {
        self.shelf.duckdb.insert_statement(key, &content, tr_start, tr_end)?;

        // Insert into FTS with CSV-formatted key
        let csv_key = key.to_csv_key();
        let fts_content = content.fts_content(&format!("{} {} {}", key.subject, key.predicate, key.object));
        self.shelf.sqlite.upsert_doc("statement", &csv_key, &fts_content)?;

        let statement = self.shelf.duckdb.get_statement(key)?.ok_or_else(|| {
            crate::error::HypatiaError::NotFound {
                kind: "statement".to_string(),
                key: csv_key,
            }
        })?;
        Ok(statement)
    }

    pub fn get(&self, key: &StatementKey) -> Result<Option<Statement>> {
        self.shelf.duckdb.get_statement(key)
    }

    pub fn update(
        &mut self,
        key: &StatementKey,
        content: Content,
        tr_start: Option<NaiveDateTime>,
        tr_end: Option<NaiveDateTime>,
    ) -> Result<Statement> {
        self.shelf.duckdb.update_statement(key, &content, tr_start, tr_end)?;

        // Update FTS
        let csv_key = key.to_csv_key();
        let fts_content = content.fts_content(&format!("{} {} {}", key.subject, key.predicate, key.object));
        self.shelf.sqlite.upsert_doc("statement", &csv_key, &fts_content)?;

        let statement = self.shelf.duckdb.get_statement(key)?.ok_or_else(|| {
            crate::error::HypatiaError::NotFound {
                kind: "statement".to_string(),
                key: csv_key,
            }
        })?;
        Ok(statement)
    }

    pub fn delete(&mut self, key: &StatementKey) -> Result<()> {
        self.shelf.duckdb.delete_statement(key)?;
        let csv_key = key.to_csv_key();
        self.shelf.sqlite.delete_doc("statement", &csv_key)?;
        Ok(())
    }
}
