use serde_json::Map;

/// AST node for Hypatia's JSE subset.
#[derive(Debug, Clone)]
pub enum AstNode {
    /// A literal JSON value (string, number, bool, null).
    Literal(serde_json::Value),

    /// A symbol reference — a string starting with $ that is NOT a recognized operator.
    Symbol(String),

    /// An operator invocation with operands and optional metadata.
    Operator {
        operator: String,
        operands: Vec<AstNode>,
        metadata: Map<String, serde_json::Value>,
    },

    /// $quote — prevents evaluation of inner expression.
    Quote(Box<AstNode>),

    /// A plain data array (first element is not a $-symbol).
    Array(Vec<AstNode>),

    /// A plain data object (no $ keys).
    Object(Map<String, serde_json::Value>),
}

/// Which table a top-level query targets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryTarget {
    Knowledge,
    Statement,
}
