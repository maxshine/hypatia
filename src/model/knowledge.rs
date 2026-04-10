use chrono::NaiveDateTime;

use super::Content;

#[derive(Debug, Clone)]
pub struct Knowledge {
    pub name: String,
    pub content: Content,
    pub created_at: NaiveDateTime,
}
