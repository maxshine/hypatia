use crate::model::QueryTarget;

/// Builds parameterized SQL queries from collected conditions.
pub struct SqlBuilder {
    target: QueryTarget,
    conditions: Vec<String>,
    params: Vec<serde_json::Value>,
    limit: i64,
    offset: i64,
}

impl SqlBuilder {
    pub fn new(target: QueryTarget) -> Self {
        Self {
            target,
            conditions: Vec::new(),
            params: Vec::new(),
            limit: 100,
            offset: 0,
        }
    }

    pub fn add_condition(&mut self, fragment: String, params: Vec<serde_json::Value>) {
        self.conditions.push(fragment);
        self.params.extend(params);
    }

    pub fn set_limit(&mut self, limit: i64) {
        self.limit = limit;
    }

    pub fn set_offset(&mut self, offset: i64) {
        self.offset = offset;
    }

    /// Build the final SQL and parameter list.
    /// Uses CAST for timestamp columns so they're read as strings.
    pub fn build(mut self) -> (String, Vec<serde_json::Value>) {
        let table = self.target.table_name();
        let select = match self.target {
            QueryTarget::Knowledge => {
                "SELECT name, content, CAST(created_at AS VARCHAR) AS created_at"
            }
            QueryTarget::Statement => {
                "SELECT subject, predicate, object, content, \
                 CAST(created_at AS VARCHAR) AS created_at, \
                 CAST(tr_start AS VARCHAR) AS tr_start, \
                 CAST(tr_end AS VARCHAR) AS tr_end"
            }
        };

        let mut sql = format!("{select} FROM {table}");

        if !self.conditions.is_empty() {
            let where_clause = self.conditions.join(" AND ");
            sql.push_str(" WHERE ");
            sql.push_str(&where_clause);
        }

        sql.push_str(" ORDER BY created_at DESC");

        // Append limit/offset as string parameters
        let limit_idx = self.params.len();
        self.params.push(serde_json::Value::Number(self.limit.into()));
        let offset_idx = self.params.len();
        self.params.push(serde_json::Value::Number(self.offset.into()));

        sql.push_str(&format!(" LIMIT CAST(?{} AS INTEGER) OFFSET CAST(?{} AS INTEGER)", limit_idx + 1, offset_idx + 1));

        (sql, self.params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_knowledge_query() {
        let mut builder = SqlBuilder::new(QueryTarget::Knowledge);
        builder.add_condition("name = ?".to_string(), vec![serde_json::json!("test")]);
        let (sql, params) = builder.build();
        assert!(sql.contains("FROM knowledge"));
        assert!(sql.contains("WHERE name = ?"));
        assert!(sql.contains("ORDER BY created_at DESC"));
        assert_eq!(params.len(), 3); // 1 condition + limit + offset
    }

    #[test]
    fn build_statement_query_with_limit() {
        let mut builder = SqlBuilder::new(QueryTarget::Statement);
        builder.set_limit(10);
        builder.set_offset(20);
        let (sql, params) = builder.build();
        assert!(sql.contains("FROM statement"));
        assert!(sql.contains("LIMIT"));
        assert!(sql.contains("OFFSET"));
        assert_eq!(params[0], serde_json::json!(10));
        assert_eq!(params[1], serde_json::json!(20));
    }

    #[test]
    fn build_no_conditions() {
        let builder = SqlBuilder::new(QueryTarget::Knowledge);
        let (sql, _) = builder.build();
        assert!(!sql.contains("WHERE"));
        assert!(sql.contains("FROM knowledge"));
    }
}
