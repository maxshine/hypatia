use crate::error::Result;
use crate::model::{Content, Knowledge};
use crate::storage::OpenShelf;

pub struct KnowledgeService<'a> {
    shelf: &'a mut OpenShelf,
}

impl<'a> KnowledgeService<'a> {
    pub fn new(shelf: &'a mut OpenShelf) -> Self {
        Self { shelf }
    }

    pub fn create(&mut self, name: &str, content: Content) -> Result<Knowledge> {
        // Insert into DuckDB
        self.shelf.duckdb.insert_knowledge(name, &content)?;

        // Insert into SQLite FTS
        let fts_content = content.fts_content(name);
        self.shelf.sqlite.upsert_doc("knowledge", name, &fts_content)?;

        // Read back to get the generated timestamp
        let knowledge = self.shelf.duckdb.get_knowledge(name)?.ok_or_else(|| {
            crate::error::HypatiaError::NotFound {
                kind: "knowledge".to_string(),
                key: name.to_string(),
            }
        })?;
        Ok(knowledge)
    }

    pub fn get(&self, name: &str) -> Result<Option<Knowledge>> {
        self.shelf.duckdb.get_knowledge(name)
    }

    pub fn update(&mut self, name: &str, content: Content) -> Result<Knowledge> {
        self.shelf.duckdb.update_knowledge(name, &content)?;

        // Update FTS
        let fts_content = content.fts_content(name);
        self.shelf.sqlite.upsert_doc("knowledge", name, &fts_content)?;

        let knowledge = self.shelf.duckdb.get_knowledge(name)?.ok_or_else(|| {
            crate::error::HypatiaError::NotFound {
                kind: "knowledge".to_string(),
                key: name.to_string(),
            }
        })?;
        Ok(knowledge)
    }

    pub fn delete(&mut self, name: &str) -> Result<()> {
        self.shelf.duckdb.delete_knowledge(name)?;
        self.shelf.sqlite.delete_doc("knowledge", name)?;
        Ok(())
    }
}
