use crate::error::Result;
use crate::model::{QueryResult, SearchOpts};
use crate::storage::{OpenShelf, Storage};

pub struct SearchService<'a> {
    shelf: &'a OpenShelf,
}

impl<'a> SearchService<'a> {
    pub fn new(shelf: &'a OpenShelf) -> Self {
        Self { shelf }
    }

    pub fn search(&self, query: &str, opts: &SearchOpts) -> Result<QueryResult> {
        self.shelf.execute_search(query, opts)
    }
}
