use crate::error::{HypatiaError, Result};
use super::ast::AstNode;

/// Result of evaluating an operator in the context of query building.
#[derive(Debug, Clone)]
pub enum OperatorResult {
    /// A SQL WHERE fragment with parameterized values.
    SqlCondition {
        fragment: String,
        params: Vec<serde_json::Value>,
    },
    /// A FTS search query string. Opts are inherited from the parent query.
    FtsQuery {
        query: String,
    },
    /// A literal value (from $quote or non-operator expressions).
    Value(serde_json::Value),
}

/// Evaluate an operator AST node against its operands.
/// Returns the SQL contribution of this operator.
pub fn evaluate_operator(
    operator: &str,
    operands: &[AstNode],
    _metadata: &serde_json::Map<String, serde_json::Value>,
    eval_fn: &dyn Fn(&AstNode) -> Result<OperatorResult>,
) -> Result<OperatorResult> {
    match operator {
        "$knowledge" | "$statement" => {
            // These are handled by the evaluator at the top level.
            // When evaluated as operators, they just pass through their first operand.
            if operands.len() == 1 {
                eval_fn(&operands[0])
            } else {
                // No conditions — return a tautology
                Ok(OperatorResult::SqlCondition {
                    fragment: "1=1".to_string(),
                    params: Vec::new(),
                })
            }
        }
        "$and" => {
            let mut fragments = Vec::new();
            let mut all_params = Vec::new();
            for operand in operands {
                match eval_fn(operand)? {
                    OperatorResult::SqlCondition { fragment, params } => {
                        fragments.push(fragment);
                        all_params.extend(params);
                    }
                    other => {
                        return Err(HypatiaError::Eval(format!(
                            "$and expects SQL conditions, got {:?}", other
                        )));
                    }
                }
            }
            if fragments.is_empty() {
                Ok(OperatorResult::SqlCondition {
                    fragment: "1=1".to_string(),
                    params: Vec::new(),
                })
            } else if fragments.len() == 1 {
                Ok(OperatorResult::SqlCondition {
                    fragment: fragments.into_iter().next().unwrap(),
                    params: all_params,
                })
            } else {
                Ok(OperatorResult::SqlCondition {
                    fragment: format!("({})", fragments.join(" AND ")),
                    params: all_params,
                })
            }
        }
        "$or" => {
            let mut fragments = Vec::new();
            let mut all_params = Vec::new();
            for operand in operands {
                match eval_fn(operand)? {
                    OperatorResult::SqlCondition { fragment, params } => {
                        fragments.push(fragment);
                        all_params.extend(params);
                    }
                    other => {
                        return Err(HypatiaError::Eval(format!(
                            "$or expects SQL conditions, got {:?}", other
                        )));
                    }
                }
            }
            if fragments.is_empty() {
                Ok(OperatorResult::SqlCondition {
                    fragment: "1=1".to_string(),
                    params: Vec::new(),
                })
            } else {
                Ok(OperatorResult::SqlCondition {
                    fragment: format!("({})", fragments.join(" OR ")),
                    params: all_params,
                })
            }
        }
        "$not" => {
            if operands.len() != 1 {
                return Err(HypatiaError::Eval("$not expects exactly one argument".to_string()));
            }
            match eval_fn(&operands[0])? {
                OperatorResult::SqlCondition { fragment, params } => {
                    Ok(OperatorResult::SqlCondition {
                        fragment: format!("NOT ({fragment})"),
                        params,
                    })
                }
                other => Err(HypatiaError::Eval(format!(
                    "$not expects SQL condition, got {:?}", other
                ))),
            }
        }
        "$eq" => comparison_op("=", operands, eval_fn),
        "$ne" => comparison_op("!=", operands, eval_fn),
        "$gt" => comparison_op(">", operands, eval_fn),
        "$lt" => comparison_op("<", operands, eval_fn),
        "$gte" => comparison_op(">=", operands, eval_fn),
        "$lte" => comparison_op("<=", operands, eval_fn),
        "$contains" => {
            if operands.len() != 2 {
                return Err(HypatiaError::Eval(
                    "$contains expects exactly two arguments (field, value)".to_string(),
                ));
            }
            let field = expect_symbol(&operands[0])?;
            let value = expect_literal(&operands[1])?;
            let search_str = match &value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            Ok(OperatorResult::SqlCondition {
                fragment: format!("json_extract_string(content, '$.{field}') LIKE ?"),
                params: vec![serde_json::Value::String(format!("%{search_str}%"))],
            })
        }
        "$search" => {
            let query = if operands.is_empty() {
                return Err(HypatiaError::Eval("$search expects a query argument".to_string()));
            } else {
                expect_literal(&operands[0])?
            };
            let query_str = match &query {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            Ok(OperatorResult::FtsQuery {
                query: query_str,
            })
        }
        "$quote" => {
            if operands.len() != 1 {
                return Err(HypatiaError::Eval("$quote expects exactly one argument".to_string()));
            }
            // Return the unevaluated operand as a literal value
            Ok(OperatorResult::Value(ast_to_value(&operands[0])))
        }
        _ => Err(HypatiaError::Eval(format!("unknown operator: {operator}"))),
    }
}

/// Handle comparison operators: $eq, $ne, $gt, $lt, $gte, $lte
fn comparison_op(
    op: &str,
    operands: &[AstNode],
    eval_fn: &dyn Fn(&AstNode) -> Result<OperatorResult>,
) -> Result<OperatorResult> {
    if operands.len() == 2 {
        // Two-argument form: ["$eq", "field", "value"]
        let field = expect_symbol(&operands[0])?;
        let value = expect_literal(&operands[1])?;
        let sql_field = resolve_field(&field);
        Ok(OperatorResult::SqlCondition {
            fragment: format!("{sql_field} {op} ?"),
            params: vec![value],
        })
    } else if operands.len() == 1 {
        // Single argument: the operand should already be a condition
        eval_fn(&operands[0])
    } else {
        Err(HypatiaError::Eval(format!(
            "comparison operator expects 1 or 2 arguments, got {}", operands.len()
        )))
    }
}

/// Extract a symbol name from an AST node.
fn expect_symbol(node: &AstNode) -> Result<String> {
    match node {
        AstNode::Symbol(s) => Ok(s.clone()),
        AstNode::Literal(serde_json::Value::String(s)) => Ok(s.clone()),
        _ => Err(HypatiaError::Eval(format!(
            "expected symbol or string, got {:?}", node
        ))),
    }
}

/// Extract a literal value from an AST node.
fn expect_literal(node: &AstNode) -> Result<serde_json::Value> {
    match node {
        AstNode::Literal(v) => Ok(v.clone()),
        AstNode::Symbol(s) => Ok(serde_json::Value::String(s.clone())),
        _ => Ok(ast_to_value(node)),
    }
}

/// Resolve a field name to its SQL column reference.
fn resolve_field(field: &str) -> String {
    let field = field.trim_start_matches('$');
    match field {
        "subject" | "predicate" | "object" | "name" | "created_at" | "tr_start" | "tr_end" => {
            field.to_string()
        }
        // Assume it's a JSON content field
        _ => format!("json_extract_string(content, '$.{field}')"),
    }
}

/// Convert an AST node back to a JSON value (for $quote).
fn ast_to_value(node: &AstNode) -> serde_json::Value {
    match node {
        AstNode::Literal(v) => v.clone(),
        AstNode::Symbol(s) => serde_json::Value::String(s.clone()),
        AstNode::Array(nodes) => {
            serde_json::Value::Array(nodes.iter().map(ast_to_value).collect())
        }
        AstNode::Object(map) => serde_json::Value::Object(map.clone()),
        AstNode::Quote(inner) => ast_to_value(inner),
        AstNode::Operator { operator, operands, metadata } => {
            let mut arr = vec![serde_json::Value::String(operator.clone())];
            arr.extend(operands.iter().map(ast_to_value));
            if metadata.is_empty() {
                serde_json::Value::Array(arr)
            } else {
                // Merge with metadata
                let mut obj = metadata.clone();
                obj.insert(operator.clone(), serde_json::Value::Array(arr[1..].to_vec()));
                serde_json::Value::Object(obj)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_eval(ops: &[(&str, Vec<AstNode>)]) -> Box<dyn Fn(&AstNode) -> Result<OperatorResult>> {
        // Simple eval that matches operator patterns
        let pairs: Vec<(String, Vec<AstNode>)> = ops
            .iter()
            .map(|(op, args)| (op.to_string(), args.clone()))
            .collect();
        Box::new(move |node: &AstNode| {
            match node {
                AstNode::Operator { operator, operands, .. } => {
                    evaluate_operator(
                        operator,
                        operands,
                        &serde_json::Map::new(),
                        &|n| {
                            // Recursive eval for nested operators
                            match n {
                                AstNode::Operator { operator, operands, .. } => {
                                    evaluate_operator(operator, operands, &serde_json::Map::new(), &|_| {
                                        Err(HypatiaError::Eval("unexpected deep nesting".to_string()))
                                    })
                                }
                                _ => Err(HypatiaError::Eval("expected operator".to_string())),
                            }
                        },
                    )
                }
                _ => Err(HypatiaError::Eval("expected operator".to_string())),
            }
        })
    }

    #[test]
    fn eq_operator() {
        let eval = make_eval(&[]);
        let result = evaluate_operator(
            "$eq",
            &[AstNode::Symbol("$name".to_string()), AstNode::Literal(json!("Alice"))],
            &serde_json::Map::new(),
            &|_| Err(HypatiaError::Eval("should not recurse".to_string())),
        ).unwrap();
        match result {
            OperatorResult::SqlCondition { fragment, params } => {
                assert!(fragment.contains("="));
                assert_eq!(params.len(), 1);
            }
            _ => panic!("expected SqlCondition"),
        }
    }

    #[test]
    fn and_operator() {
        let result = evaluate_operator(
            "$and",
            &[
                AstNode::Operator {
                    operator: "$eq".to_string(),
                    operands: vec![AstNode::Symbol("$name".to_string()), AstNode::Literal(json!("test"))],
                    metadata: serde_json::Map::new(),
                },
                AstNode::Operator {
                    operator: "$gt".to_string(),
                    operands: vec![AstNode::Symbol("$age".to_string()), AstNode::Literal(json!(18))],
                    metadata: serde_json::Map::new(),
                },
            ],
            &serde_json::Map::new(),
            &|node: &AstNode| {
                match node {
                    AstNode::Operator { operator, operands, .. } => {
                        evaluate_operator(operator, operands, &serde_json::Map::new(), &|_| {
                            Err(HypatiaError::Eval("no deeper nesting".to_string()))
                        })
                    }
                    _ => Err(HypatiaError::Eval("expected operator".to_string())),
                }
            },
        ).unwrap();
        match result {
            OperatorResult::SqlCondition { fragment, params } => {
                assert!(fragment.contains("AND"));
                assert_eq!(params.len(), 2);
            }
            _ => panic!("expected SqlCondition"),
        }
    }

    #[test]
    fn search_operator() {
        let result = evaluate_operator(
            "$search",
            &[AstNode::Literal(json!("hello world"))],
            &serde_json::Map::new(),
            &|_| Err(HypatiaError::Eval("should not recurse".to_string())),
        ).unwrap();
        match result {
            OperatorResult::FtsQuery { query } => {
                assert_eq!(query, "hello world");
            }
            _ => panic!("expected FtsQuery"),
        }
    }

    #[test]
    fn contains_operator() {
        let result = evaluate_operator(
            "$contains",
            &[AstNode::Symbol("$tags".to_string()), AstNode::Literal(json!("rust"))],
            &serde_json::Map::new(),
            &|_| Err(HypatiaError::Eval("should not recurse".to_string())),
        ).unwrap();
        match result {
            OperatorResult::SqlCondition { fragment, params } => {
                assert!(fragment.contains("json_extract_string"));
                assert!(fragment.contains("LIKE"));
                assert_eq!(params[0], json!("%rust%"));
            }
            _ => panic!("expected SqlCondition"),
        }
    }

    #[test]
    fn not_operator() {
        let result = evaluate_operator(
            "$not",
            &[AstNode::Operator {
                operator: "$eq".to_string(),
                operands: vec![AstNode::Symbol("$name".to_string()), AstNode::Literal(json!("test"))],
                metadata: serde_json::Map::new(),
            }],
            &serde_json::Map::new(),
            &|node: &AstNode| {
                match node {
                    AstNode::Operator { operator, operands, .. } => {
                        evaluate_operator(operator, operands, &serde_json::Map::new(), &|_| {
                            Err(HypatiaError::Eval("no deeper".to_string()))
                        })
                    }
                    _ => Err(HypatiaError::Eval("expected operator".to_string())),
                }
            },
        ).unwrap();
        match result {
            OperatorResult::SqlCondition { fragment, .. } => {
                assert!(fragment.starts_with("NOT ("));
            }
            _ => panic!("expected SqlCondition"),
        }
    }
}
