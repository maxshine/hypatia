---
name: hypatia-query
description: "Interact with the Hypatia AI memory system using natural language. Translate user requests into hypatia CLI commands for knowledge CRUD, statement (RDF triple) management, JSE queries, full-text search, and shelf management. Trigger when: user mentions memories, knowledge bases, knowledge graphs, triples, statements, relationships, shelves, or asks to store, recall, remember, record, save, find, or search information in hypatia. Also trigger when user wants to create or explore relationships between concepts, query existing knowledge, or manage shelves. Examples: 'remember that Rust is a systems language', 'find everything about Alice', 'record that Alice knows Bob', 'search for programming', 'show all knowledge', 'list shelves'."
user-invocable: true
allowed-tools: Bash, Read, Grep, Glob
argument-hint: <natural-language instruction>
---

# Hypatia Query Skill

You are operating the Hypatia CLI — an AI-oriented memory management system. Translate the user's natural language request into the appropriate `hypatia` CLI command and execute it via Bash.

## Binary Location

First check which binary is available:

1. `hypatia` — if installed on PATH
2. `./target/debug/hypatia` — debug build in current project
3. `./target/release/hypatia` — release build in current project

Use the first one found. All examples below use `hypatia` for brevity.

## Shelf Management

| User says | Command |
|---|---|
| "list shelves" / "show connected shelves" | `hypatia list` |
| "connect shelf at PATH" / "open data at PATH" | `hypatia connect <path> [-n <name>]` |
| "disconnect shelf NAME" / "close shelf NAME" | `hypatia disconnect <name>` |
| "export shelf NAME to DEST" | `hypatia export <name> <dest>` |

## Knowledge CRUD

Knowledge entries are independent information points with a name, content, and tags.

### Create

```
hypatia knowledge-create <name> -d "<data>" -t "<tag1,tag2>"
```

| User says | Command |
|---|---|
| "remember Rust as a systems programming language" | `hypatia knowledge-create "Rust" -d "systems programming language"` |
| "save knowledge about Go with tags language and compiled" | `hypatia knowledge-create "Go" -d "" -t "language,compiled"` |
| "store that Python is a scripting language, tag it as dynamic" | `hypatia knowledge-create "Python" -d "scripting language" -t "dynamic"` |

### Read

```
hypatia knowledge-get <name>
```

| User says | Command |
|---|---|
| "show me knowledge about Rust" / "get Rust entry" | `hypatia knowledge-get "Rust"` |

### Delete

```
hypatia knowledge-delete <name>
```

| User says | Command |
|---|---|
| "delete knowledge Rust" / "remove the Rust entry" | `hypatia knowledge-delete "Rust"` |

## Statement Creation

Statements are RDF-style triples: `(subject, predicate, object)`.

```
hypatia statement-create <subject> <predicate> <object> -d "<data>"
```

### Triple Extraction Patterns

| User says | subject | predicate | object |
|---|---|---|---|
| "record that Alice knows Bob" | Alice | knows | Bob |
| "X is a Y" / "X is an Y" | X | is_a | Y |
| "X belongs to Y" / "X is part of Y" | X | belongs_to | Y |
| "X works for Y" | X | works_for | Y |
| "X is related to Y" | X | related_to | Y |
| "X depends on Y" | X | depends_on | Y |

Normalize predicates to `snake_case`. Common predicates: `is_a`, `knows`, `related_to`, `works_for`, `belongs_to`, `depends_on`, `uses`, `contains`, `created_by`.

### Examples

| User says | Command |
|---|---|
| "record that Alice knows Bob" | `hypatia statement-create "Alice" "knows" "Bob"` |
| "remember that Rust is a systems language" | `hypatia statement-create "Rust" "is_a" "systems language"` |
| "note that the API depends on PostgreSQL, with context 'critical dependency'" | `hypatia statement-create "API" "depends_on" "PostgreSQL" -d "critical dependency"` |

## Full-text Search

Search is the safest default for broad or ambiguous queries. It covers both knowledge and statements.

```
hypatia search <query> [-c <catalog>] [--limit N] [--offset N]
```

- `-c knowledge` — search only knowledge entries
- `-c statement` — search only statements
- No `-c` — search everything

### Examples

| User says | Command |
|---|---|
| "search for programming" / "find everything about programming" | `hypatia search "programming"` |
| "search knowledge for rust" | `hypatia search "rust" -c knowledge` |
| "find statements about Alice" | `hypatia search "Alice" -c statement` |
| "what do you know about databases?" | `hypatia search "databases"` |

## JSE Query Translation

JSE (JSON Search Expression) enables precise queries against the knowledge or statement tables.

```
hypatia query '<JSE-JSON>' [-s <shelf>]
```

### Query Structure

The top-level operator is always `$knowledge` or `$statement`:

```json
["$knowledge", condition1, condition2, ...]
["$statement", condition1, condition2, ...]
```

No conditions means "return all":

```json
["$knowledge"]
```

### Operator Reference

| Operator | Purpose | Syntax |
|---|---|---|
| `$eq` | Equals | `["$eq", "field", "value"]` |
| `$ne` | Not equals | `["$ne", "field", "value"]` |
| `$gt` | Greater than | `["$gt", "field", "value"]` |
| `$lt` | Less than | `["$lt", "field", "value"]` |
| `$gte` | Greater than or equal | `["$gte", "field", "value"]` |
| `$lte` | Less than or equal | `["$lte", "field", "value"]` |
| `$contains` | Substring match in content JSON | `["$contains", "field", "value"]` |
| `$search` | Full-text search (FTS) | `["$search", "query text"]` |
| `$and` | Logical AND | `["$and", cond1, cond2, ...]` |
| `$or` | Logical OR | `["$or", cond1, cond2, ...]` |
| `$not` | Logical NOT | `["$not", condition]` |

### Critical Syntax Rules

1. **`$and` and `$or` take multiple operands directly**, NOT a nested array:
   - CORRECT: `["$and", ["$eq", "name", "rust"], ["$contains", "tags", "systems"]]`
   - WRONG: `["$and", [["$eq", "name", "rust"], ["$contains", "tags", "systems"]]]`

2. **`$search` must be inside `$knowledge` or `$statement`**, never top-level. When inside `$knowledge`, it searches FTS with catalog=knowledge. When inside `$statement`, it searches with catalog=statement.

3. **Field names** like `"name"`, `"subject"`, `"predicate"`, `"object"` are used as plain strings. For content JSON sub-fields (e.g., tags), `$contains` uses `json_extract_string` automatically.

4. **Available fields for knowledge**: `name`, `created_at`, plus any content JSON field via `$contains`.

5. **Available fields for statement**: `subject`, `predicate`, `object`, `created_at`, `tr_start`, `tr_end`, plus content JSON fields via `$contains`.

### Natural Language to JSE Examples

| User says | JSE | Command |
|---|---|---|
| "find knowledge named rust" | `["$knowledge", ["$eq", "name", "rust"]]` | `hypatia query '["$knowledge", ["$eq", "name", "rust"]]'` |
| "search knowledge about rust programming" | `["$knowledge", ["$search", "rust programming"]]` | `hypatia query '["$knowledge", ["$search", "rust programming"]]'` |
| "find statements where Alice is the subject" | `["$statement", ["$eq", "subject", "Alice"]]` | `hypatia query '["$statement", ["$eq", "subject", "Alice"]]'` |
| "find statements about Alice (full-text)" | `["$statement", ["$search", "Alice"]]` | `hypatia query '["$statement", ["$search", "Alice"]]'` |
| "find knowledge named rust that contains 'systems' in tags" | `["$knowledge", ["$and", ["$eq", "name", "rust"], ["$contains", "tags", "systems"]]]` | `hypatia query '["$knowledge", ["$and", ["$eq", "name", "rust"], ["$contains", "tags", "systems"]]]'` |
| "find knowledge NOT named rust" | `["$knowledge", ["$not", ["$eq", "name", "rust"]]]` | `hypatia query '["$knowledge", ["$not", ["$eq", "name", "rust"]]]'` |
| "find statements where subject is Alice or Bob" | `["$statement", ["$or", ["$eq", "subject", "Alice"], ["$eq", "subject", "Bob"]]]` | `hypatia query '["$statement", ["$or", ["$eq", "subject", "Alice"], ["$eq", "subject", "Bob"]]]'` |
| "show all knowledge" | `["$knowledge"]` | `hypatia query '["$knowledge"]'` |
| "find knowledge created after 2025-01-01" | `["$knowledge", ["$gt", "created_at", "2025-01-01"]]` | `hypatia query '["$knowledge", ["$gt", "created_at", "2025-01-01"]]'` |
| "find knowledge containing 'language' in data" | `["$knowledge", ["$contains", "data", "language"]]` | `hypatia query '["$knowledge", ["$contains", "data", "language"]]'` |

### Options (limit, offset, catalog)

Use object form for the top-level operator to pass options:

```json
{"$knowledge": [["$search", "rust"]], "limit": 10, "offset": 0}
```

Command: `hypatia query '{"$knowledge": [["$search", "rust"]], "limit": 10}'`

## Disambiguation Rules

When the user's request is ambiguous, follow this priority:

1. **Exact name mentioned** ("get Rust", "show Alice") → `knowledge-get` for direct lookup
2. **Explicit type** ("find knowledge about X" / "find statements about X") → JSE query with the corresponding top-level operator
3. **Relationship language** ("who does Alice know?", "what is Rust related to?", "relationships of X") → JSE `$statement` query
4. **Broad/ambiguous** ("find everything about X", "what do you know about X", "search for X") → `hypatia search` (covers both knowledge and statements)
5. **Create/store/remember** → knowledge-create or statement-create based on context (if it expresses a relationship between two things, use statement; otherwise knowledge)

## Shell Escaping

Always wrap JSE JSON in **single quotes** to prevent shell interpretation of `$`, `"`, `!`, etc.:

```bash
hypatia query '["$knowledge", ["$eq", "name", "rust"]]'
```

If the content contains single quotes, escape them:

```bash
hypatia query '["$knowledge", ["$eq", "name", "Alice'\''s project"]]'
```

## Output Format

- **Knowledge entries**: `{"name": "...", "content": {"format": "...", "data": "...", "tags": [...]}, "created_at": "..."}`
- **Statements**: `{"subject": "...", "predicate": "...", "object": "...", "content": {...}, "created_at": "...", "tr_start": "...", "tr_end": "..."}`
- **Search results**: `{"id": N, "catalog": "...", "key": "...", "content": "...", "rank": N.N}`
- **Empty results**: prints "No results found."

## Important Notes

- There are no `statement-get` or `statement-delete` CLI commands. Use JSE queries (`$statement` with conditions) to find statements.
- The `-s` / `--shelf` flag defaults to `"default"` for all commands.
- The REPL mode (`hypatia repl`) is interactive and should NOT be used from this skill. Always use one-shot CLI commands.
- Content (`-d`) is stored as a string in the content JSON's `data` field.
- Tags (`-t`) are comma-separated strings.
