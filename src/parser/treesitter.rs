use anyhow::Result;
use tree_sitter::{Parser, Node};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use crate::graph::{Entity, Relationship};
use super::ParseResult;

fn make_entity(
    entity_type: &str,
    name: &str,
    qualified_name: &str,
    file_path: &str,
    start_line: u32,
    end_line: u32,
    properties: serde_json::Value,
    content_hash: Option<&str>,
) -> Entity {
    Entity {
        id: Uuid::new_v4().to_string(),
        entity_type: entity_type.to_string(),
        name: name.to_string(),
        qualified_name: qualified_name.to_string(),
        file_path: Some(file_path.to_string()),
        start_line: Some(start_line),
        end_line: Some(end_line),
        properties,
        source_type: "indexed".to_string(),
        verification_status: "unverified".to_string(),
        confidence: 1.0,
        memory_scope: "persistent".to_string(),
        content_hash: content_hash.map(|s| s.to_string()),
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn make_relationship(
    source_id: &str,
    target_id: &str,
    rel_type: &str,
    properties: serde_json::Value,
) -> Relationship {
    Relationship {
        id: Uuid::new_v4().to_string(),
        source_id: source_id.to_string(),
        target_id: target_id.to_string(),
        relationship_type: rel_type.to_string(),
        properties,
        source_type: "indexed".to_string(),
        verification_status: "unverified".to_string(),
        confidence: 1.0,
        weight: 1.0,
        memory_scope: "persistent".to_string(),
    }
}

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

const SKIP_CALL_TARGETS: &[&str] = &[
    "console.log", "console.error", "console.warn", "console.info", "console.debug",
    "require", "setTimeout", "setInterval", "clearTimeout", "clearInterval",
    "Promise.resolve", "Promise.reject", "Promise.all", "Promise.race",
    "JSON.stringify", "JSON.parse", "Object.keys", "Object.values", "Object.assign",
    "Object.entries", "Array.isArray", "Array.from",
    "parseInt", "parseFloat", "String", "Number", "Boolean",
    "print", "len", "range", "enumerate", "zip", "map", "filter",
    "isinstance", "issubclass", "getattr", "setattr", "hasattr",
    "super", "type", "str", "int", "float", "bool", "list", "dict", "set", "tuple",
];

pub fn parse_typescript(source: &str, file_path: &str) -> Result<ParseResult> {
    let mut parser = Parser::new();
    let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
    parser.set_language(&language.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

    let lines: Vec<&str> = source.lines().collect();
    let loc = lines.len() as u32;
    let hash = content_hash(source);

    let file_entity = make_entity(
        "file",
        file_path.rsplit('/').next().unwrap_or(file_path),
        file_path,
        file_path,
        1,
        loc,
        serde_json::json!({
            "path": file_path,
            "language": "typescript",
            "loc": loc,
            "hash": hash,
        }),
        Some(&hash),
    );

    let mut entities = vec![file_entity.clone()];
    let mut relationships = Vec::new();

    let root = tree.root_node();
    extract_ts_entities(
        root,
        source,
        file_path,
        &file_entity.id,
        &mut entities,
        &mut relationships,
    );

    // Pass 2: extract call relationships from all function entities
    let function_entities: Vec<(String, String, Option<u32>, Option<u32>)> = entities
        .iter()
        .filter(|e| e.entity_type == "function")
        .map(|e| (e.id.clone(), e.qualified_name.clone(), e.start_line, e.end_line))
        .collect();

    for (fn_id, fn_qname, start, end) in &function_entities {
        if let (Some(start_line), Some(end_line)) = (start, end) {
            extract_ts_calls_in_range(
                root,
                source,
                file_path,
                &fn_id,
                &fn_qname,
                *start_line,
                *end_line,
                &mut relationships,
            );
        }
    }

    Ok(ParseResult {
        entities,
        relationships,
    })
}

fn extract_ts_entities(
    node: Node,
    source: &str,
    file_path: &str,
    file_entity_id: &str,
    entities: &mut Vec<Entity>,
    relationships: &mut Vec<Relationship>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "method_definition" => {
                if let Some(entity) = extract_ts_function(child, source, file_path) {
                    let rel = make_relationship(
                        file_entity_id,
                        &entity.id,
                        "contains",
                        serde_json::json!({}),
                    );
                    relationships.push(rel);
                    entities.push(entity);
                }
            }
            "class_declaration" => {
                if let Some(entity) = extract_ts_class(child, source, file_path) {
                    let class_id = entity.id.clone();
                    let rel = make_relationship(
                        file_entity_id,
                        &class_id,
                        "contains",
                        serde_json::json!({}),
                    );
                    relationships.push(rel);
                    entities.push(entity);

                    let body = child.child_by_field_name("body");
                    if let Some(body_node) = body {
                        extract_ts_class_members(
                            body_node,
                            source,
                            file_path,
                            &class_id,
                            entities,
                            relationships,
                        );
                    }
                }
            }
            "export_statement" => {
                extract_ts_entities(child, source, file_path, file_entity_id, entities, relationships);
            }
            "lexical_declaration" | "variable_declaration" => {
                extract_ts_variables(child, source, file_path, file_entity_id, entities, relationships);
            }
            "import_statement" => {
                if let Some(import_path) = extract_import_path(child, source) {
                    let import_entity = make_entity(
                        "dependency",
                        &import_path,
                        &format!("{}::import::{}", file_path, import_path),
                        file_path,
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                        serde_json::json!({ "import_path": import_path }),
                        None,
                    );
                    let rel = make_relationship(
                        file_entity_id,
                        &import_entity.id,
                        "imports",
                        serde_json::json!({}),
                    );
                    relationships.push(rel);
                    entities.push(import_entity);
                }
            }
            "type_alias_declaration" | "interface_declaration" | "enum_declaration" => {
                if let Some(entity) = extract_ts_type_def(child, source, file_path) {
                    let rel = make_relationship(
                        file_entity_id,
                        &entity.id,
                        "contains",
                        serde_json::json!({}),
                    );
                    relationships.push(rel);
                    entities.push(entity);
                }
            }
            _ => {
                extract_ts_entities(child, source, file_path, file_entity_id, entities, relationships);
            }
        }
    }
}

fn extract_ts_function(node: Node, source: &str, file_path: &str) -> Option<Entity> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?;
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;

    let params_text = node.child_by_field_name("parameters")
        .and_then(|p| p.utf8_text(source.as_bytes()).ok())
        .unwrap_or("()");

    let return_type = node.child_by_field_name("return_type")
        .and_then(|r| r.utf8_text(source.as_bytes()).ok())
        .unwrap_or("");

    let is_async = node.child(0).map(|c| c.kind() == "async").unwrap_or(false);
    let is_exported = node.parent().map(|p| p.kind() == "export_statement").unwrap_or(false);

    let signature = format!(
        "{}function {}{}{}",
        if is_async { "async " } else { "" },
        name,
        params_text,
        if return_type.is_empty() { String::new() } else { format!(": {}", return_type) }
    );

    let params = extract_ts_parameters(node, source);

    let body_text = node.utf8_text(source.as_bytes()).unwrap_or("");
    let body_hash = content_hash(body_text);

    Some(make_entity(
        "function",
        name,
        &format!("{}::{}", file_path, name),
        file_path,
        start_line,
        end_line,
        serde_json::json!({
            "name": name,
            "signature": signature,
            "parameters": params,
            "is_async": is_async,
            "is_exported": is_exported,
            "visibility": if is_exported { "public" } else { "private" },
            "loc": end_line - start_line + 1,
            "start_line": start_line,
            "end_line": end_line,
            "body_hash": body_hash,
        }),
        Some(&body_hash),
    ))
}

fn extract_ts_parameters(node: Node, source: &str) -> Vec<serde_json::Value> {
    let mut params = Vec::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "required_parameter" || child.kind() == "optional_parameter" {
                if let Some(name_node) = child.child_by_field_name("pattern") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        let type_ann = child
                            .child_by_field_name("type")
                            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("");
                        params.push(serde_json::json!({
                            "name": name,
                            "type": type_ann,
                        }));
                    }
                }
            }
        }
    }
    params
}

fn extract_ts_class(node: Node, source: &str, file_path: &str) -> Option<Entity> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?;
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    let is_exported = node.parent().map(|p| p.kind() == "export_statement").unwrap_or(false);

    Some(make_entity(
        "class",
        name,
        &format!("{}::{}", file_path, name),
        file_path,
        start_line,
        end_line,
        serde_json::json!({
            "name": name,
            "is_exported": is_exported,
            "visibility": if is_exported { "public" } else { "private" },
            "loc": end_line - start_line + 1,
            "start_line": start_line,
            "end_line": end_line,
        }),
        None,
    ))
}

fn extract_ts_class_members(
    body: Node,
    source: &str,
    file_path: &str,
    class_id: &str,
    entities: &mut Vec<Entity>,
    relationships: &mut Vec<Relationship>,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "method_definition" {
            if let Some(entity) = extract_ts_function(child, source, file_path) {
                let rel = make_relationship(
                    class_id,
                    &entity.id,
                    "contains",
                    serde_json::json!({}),
                );
                relationships.push(rel);
                entities.push(entity);
            }
        }
    }
}

fn extract_ts_variables(
    node: Node,
    source: &str,
    file_path: &str,
    parent_id: &str,
    entities: &mut Vec<Entity>,
    relationships: &mut Vec<Relationship>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    let is_const = node
                        .child(0)
                        .map(|c| c.utf8_text(source.as_bytes()).unwrap_or("") == "const")
                        .unwrap_or(false);
                    let is_exported = node.parent().map(|p| p.kind() == "export_statement").unwrap_or(false);
                    let start_line = child.start_position().row as u32 + 1;
                    let end_line = child.end_position().row as u32 + 1;

                    let value_node = child.child_by_field_name("value");
                    let is_arrow_fn = value_node
                        .map(|v| v.kind() == "arrow_function")
                        .unwrap_or(false);

                    if is_arrow_fn {
                        let arrow = value_node.unwrap();
                        let fn_end = arrow.end_position().row as u32 + 1;
                        let body_text = arrow.utf8_text(source.as_bytes()).unwrap_or("");
                        let body_hash = content_hash(body_text);

                        let is_async = {
                            let mut c = arrow.walk();
                            let result = arrow.children(&mut c).any(|ch| ch.kind() == "async");
                            result
                        };

                        let params_text = arrow.child_by_field_name("parameters")
                            .and_then(|p| p.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("()");

                        let return_type = arrow.child_by_field_name("return_type")
                            .and_then(|r| r.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("");

                        let signature = format!(
                            "const {} = {}{}{} => ...",
                            name,
                            if is_async { "async " } else { "" },
                            params_text,
                            if return_type.is_empty() { String::new() } else { format!(": {}", return_type) }
                        );

                        let params = extract_arrow_parameters(arrow, source);

                        let entity = make_entity(
                            "function",
                            name,
                            &format!("{}::{}", file_path, name),
                            file_path,
                            start_line,
                            fn_end,
                            serde_json::json!({
                                "name": name,
                                "signature": signature,
                                "parameters": params,
                                "is_async": is_async,
                                "is_exported": is_exported,
                                "is_arrow": true,
                                "visibility": if is_exported { "public" } else { "private" },
                                "loc": fn_end - start_line + 1,
                                "start_line": start_line,
                                "end_line": fn_end,
                                "body_hash": body_hash,
                            }),
                            Some(&body_hash),
                        );
                        let rel = make_relationship(parent_id, &entity.id, "contains", serde_json::json!({}));
                        relationships.push(rel);
                        entities.push(entity);
                    } else {
                        let entity = make_entity(
                            "variable",
                            name,
                            &format!("{}::{}", file_path, name),
                            file_path,
                            start_line,
                            end_line,
                            serde_json::json!({
                                "name": name,
                                "is_const": is_const,
                                "is_exported": is_exported,
                                "scope": "module",
                            }),
                            None,
                        );
                        let rel = make_relationship(parent_id, &entity.id, "contains", serde_json::json!({}));
                        relationships.push(rel);
                        entities.push(entity);
                    }
                }
            }
        }
    }
}

fn extract_arrow_parameters(arrow: Node, source: &str) -> Vec<serde_json::Value> {
    let mut params = Vec::new();
    if let Some(params_node) = arrow.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "required_parameter" | "optional_parameter" => {
                    let pname = child.child_by_field_name("pattern")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("");
                    let ptype = child.child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("");
                    if !pname.is_empty() {
                        params.push(serde_json::json!({ "name": pname, "type": ptype }));
                    }
                }
                "identifier" => {
                    if let Ok(pname) = child.utf8_text(source.as_bytes()) {
                        if pname != "(" && pname != ")" && pname != "," {
                            params.push(serde_json::json!({ "name": pname, "type": "" }));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    params
}

/// Walk a function's AST range looking for call_expression nodes (Bug 3).
fn extract_ts_calls_in_range(
    root: Node,
    source: &str,
    file_path: &str,
    caller_id: &str,
    caller_qname: &str,
    fn_start: u32,
    fn_end: u32,
    relationships: &mut Vec<Relationship>,
) {
    let mut seen_callees = std::collections::HashSet::new();
    collect_calls_recursive(root, source, file_path, caller_id, caller_qname,
                           fn_start, fn_end, relationships, &mut seen_callees);
}

fn collect_calls_recursive(
    node: Node,
    source: &str,
    file_path: &str,
    caller_id: &str,
    caller_qname: &str,
    fn_start: u32,
    fn_end: u32,
    relationships: &mut Vec<Relationship>,
    seen: &mut std::collections::HashSet<String>,
) {
    let node_start = node.start_position().row as u32 + 1;
    let node_end = node.end_position().row as u32 + 1;

    // Only look inside the function's line range
    if node_end < fn_start || node_start > fn_end {
        return;
    }

    if node.kind() == "call_expression" {
        if let Some(fn_node) = node.child_by_field_name("function") {
            if let Ok(callee_text) = fn_node.utf8_text(source.as_bytes()) {
                let callee = callee_text.trim();
                if !callee.is_empty()
                    && !SKIP_CALL_TARGETS.contains(&callee)
                    && !seen.contains(callee)
                {
                    seen.insert(callee.to_string());

                    // Create a "calls" relationship. The target_id is a placeholder
                    // qualified name — the indexer's relationship resolution will
                    // remap it using the entity lookup tables.
                    let target_qname = format!("{}::{}", file_path, callee.split('.').last().unwrap_or(callee));

                    relationships.push(make_relationship(
                        caller_id,
                        &target_qname,
                        "calls",
                        serde_json::json!({
                            "callee_raw": callee,
                            "call_line": node.start_position().row + 1,
                        }),
                    ));
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls_recursive(child, source, file_path, caller_id, caller_qname,
                               fn_start, fn_end, relationships, seen);
    }
}

fn extract_import_path(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string" || child.kind() == "string_fragment" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                return Some(text.trim_matches(|c| c == '\'' || c == '"').to_string());
            }
        }
        if child.kind() == "import_clause" || child.kind() == "from_clause" {
            if let Some(path) = extract_import_path(child, source) {
                return Some(path);
            }
        }
    }
    None
}

fn extract_ts_type_def(node: Node, source: &str, file_path: &str) -> Option<Entity> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?;
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    let is_exported = node.parent().map(|p| p.kind() == "export_statement").unwrap_or(false);

    let kind = match node.kind() {
        "interface_declaration" => "interface",
        "type_alias_declaration" => "type_alias",
        "enum_declaration" => "enum",
        _ => "unknown",
    };

    let definition = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();

    Some(make_entity(
        "type_definition",
        name,
        &format!("{}::{}", file_path, name),
        file_path,
        start_line,
        end_line,
        serde_json::json!({
            "name": name,
            "kind": kind,
            "is_exported": is_exported,
            "definition": definition,
            "start_line": start_line,
            "end_line": end_line,
        }),
        None,
    ))
}

// ====== PYTHON PARSER ======

pub fn parse_python(source: &str, file_path: &str) -> Result<ParseResult> {
    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser.set_language(&language.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

    let lines: Vec<&str> = source.lines().collect();
    let loc = lines.len() as u32;
    let hash = content_hash(source);

    let file_entity = make_entity(
        "file",
        file_path.rsplit('/').next().unwrap_or(file_path),
        file_path,
        file_path,
        1,
        loc,
        serde_json::json!({
            "path": file_path,
            "language": "python",
            "loc": loc,
            "hash": hash,
        }),
        Some(&hash),
    );

    let mut entities = vec![file_entity.clone()];
    let mut relationships = Vec::new();

    let root = tree.root_node();
    extract_py_entities(
        root,
        source,
        file_path,
        &file_entity.id,
        &mut entities,
        &mut relationships,
    );

    // Pass 2: extract call relationships from Python functions
    let function_entities: Vec<(String, String, Option<u32>, Option<u32>)> = entities
        .iter()
        .filter(|e| e.entity_type == "function")
        .map(|e| (e.id.clone(), e.qualified_name.clone(), e.start_line, e.end_line))
        .collect();

    for (fn_id, fn_qname, start, end) in &function_entities {
        if let (Some(start_line), Some(end_line)) = (start, end) {
            extract_py_calls_in_range(
                root, source, file_path, &fn_id, &fn_qname,
                *start_line, *end_line, &mut relationships,
            );
        }
    }

    Ok(ParseResult {
        entities,
        relationships,
    })
}

fn extract_py_entities(
    node: Node,
    source: &str,
    file_path: &str,
    parent_id: &str,
    entities: &mut Vec<Entity>,
    relationships: &mut Vec<Relationship>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(entity) = extract_py_function(child, source, file_path) {
                    let rel = make_relationship(
                        parent_id,
                        &entity.id,
                        "contains",
                        serde_json::json!({}),
                    );
                    relationships.push(rel);
                    entities.push(entity);
                }
            }
            "class_definition" => {
                if let Some(entity) = extract_py_class(child, source, file_path) {
                    let class_id = entity.id.clone();
                    let rel = make_relationship(
                        parent_id,
                        &class_id,
                        "contains",
                        serde_json::json!({}),
                    );
                    relationships.push(rel);
                    entities.push(entity);

                    if let Some(body) = child.child_by_field_name("body") {
                        extract_py_entities(body, source, file_path, &class_id, entities, relationships);
                    }
                }
            }
            "import_statement" | "import_from_statement" => {
                if let Some(import_name) = extract_py_import(child, source) {
                    let entity = make_entity(
                        "dependency",
                        &import_name,
                        &format!("{}::import::{}", file_path, import_name),
                        file_path,
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                        serde_json::json!({ "import_path": import_name }),
                        None,
                    );
                    let rel = make_relationship(parent_id, &entity.id, "imports", serde_json::json!({}));
                    relationships.push(rel);
                    entities.push(entity);
                }
            }
            "decorated_definition" => {
                extract_py_entities(child, source, file_path, parent_id, entities, relationships);
            }
            _ => {}
        }
    }
}

fn extract_py_function(node: Node, source: &str, file_path: &str) -> Option<Entity> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?;
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;

    let is_async = node.child(0).map(|c| c.kind() == "async").unwrap_or(false)
        || node.parent().and_then(|p| p.child(0)).map(|c| c.kind() == "async").unwrap_or(false);

    let params = node
        .child_by_field_name("parameters")
        .and_then(|p| p.utf8_text(source.as_bytes()).ok())
        .unwrap_or("()");

    let return_type = node
        .child_by_field_name("return_type")
        .and_then(|r| r.utf8_text(source.as_bytes()).ok())
        .unwrap_or("");

    let signature = format!(
        "{}def {}{}{}",
        if is_async { "async " } else { "" },
        name,
        params,
        if return_type.is_empty() {
            String::new()
        } else {
            format!(" -> {}", return_type)
        }
    );

    let body_text = node.utf8_text(source.as_bytes()).unwrap_or("");
    let body_hash = content_hash(body_text);

    let docstring = node
        .child_by_field_name("body")
        .and_then(|body| body.child(0))
        .filter(|c| c.kind() == "expression_statement")
        .and_then(|es| es.child(0))
        .filter(|c| c.kind() == "string")
        .and_then(|s| s.utf8_text(source.as_bytes()).ok())
        .map(|s| s.trim_matches(|c| c == '"' || c == '\'').to_string());

    Some(make_entity(
        "function",
        name,
        &format!("{}::{}", file_path, name),
        file_path,
        start_line,
        end_line,
        serde_json::json!({
            "name": name,
            "signature": signature,
            "is_async": is_async,
            "is_exported": !name.starts_with('_'),
            "visibility": if name.starts_with('_') { "private" } else { "public" },
            "loc": end_line - start_line + 1,
            "start_line": start_line,
            "end_line": end_line,
            "body_hash": body_hash,
            "docstring": docstring,
        }),
        Some(&body_hash),
    ))
}

fn extract_py_class(node: Node, source: &str, file_path: &str) -> Option<Entity> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?;
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;

    let superclasses = node
        .child_by_field_name("superclasses")
        .and_then(|s| s.utf8_text(source.as_bytes()).ok())
        .unwrap_or("");

    Some(make_entity(
        "class",
        name,
        &format!("{}::{}", file_path, name),
        file_path,
        start_line,
        end_line,
        serde_json::json!({
            "name": name,
            "is_exported": !name.starts_with('_'),
            "extends": superclasses,
            "visibility": if name.starts_with('_') { "private" } else { "public" },
            "loc": end_line - start_line + 1,
            "start_line": start_line,
            "end_line": end_line,
        }),
        None,
    ))
}

fn extract_py_import(node: Node, source: &str) -> Option<String> {
    if node.kind() == "import_from_statement" {
        let module = node.child_by_field_name("module_name")
            .and_then(|m| m.utf8_text(source.as_bytes()).ok());
        return module.map(|m| m.to_string());
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "dotted_name" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                return Some(text.to_string());
            }
        }
    }
    None
}

/// Walk a Python function's AST range looking for call nodes.
fn extract_py_calls_in_range(
    root: Node,
    source: &str,
    file_path: &str,
    caller_id: &str,
    caller_qname: &str,
    fn_start: u32,
    fn_end: u32,
    relationships: &mut Vec<Relationship>,
) {
    let mut seen = std::collections::HashSet::new();
    collect_py_calls_recursive(root, source, file_path, caller_id, caller_qname,
                               fn_start, fn_end, relationships, &mut seen);
}

fn collect_py_calls_recursive(
    node: Node,
    source: &str,
    file_path: &str,
    caller_id: &str,
    caller_qname: &str,
    fn_start: u32,
    fn_end: u32,
    relationships: &mut Vec<Relationship>,
    seen: &mut std::collections::HashSet<String>,
) {
    let node_start = node.start_position().row as u32 + 1;
    let node_end = node.end_position().row as u32 + 1;

    if node_end < fn_start || node_start > fn_end {
        return;
    }

    if node.kind() == "call" {
        if let Some(fn_node) = node.child_by_field_name("function") {
            if let Ok(callee_text) = fn_node.utf8_text(source.as_bytes()) {
                let callee = callee_text.trim();
                if !callee.is_empty()
                    && !SKIP_CALL_TARGETS.contains(&callee)
                    && !seen.contains(callee)
                {
                    seen.insert(callee.to_string());

                    let target_qname = format!("{}::{}", file_path, callee.split('.').last().unwrap_or(callee));
                    relationships.push(make_relationship(
                        caller_id,
                        &target_qname,
                        "calls",
                        serde_json::json!({
                            "callee_raw": callee,
                            "call_line": node.start_position().row + 1,
                        }),
                    ));
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_py_calls_recursive(child, source, file_path, caller_id, caller_qname,
                                   fn_start, fn_end, relationships, seen);
    }
}

// ====== GENERIC PARSER (fallback) ======

pub fn parse_generic(source: &str, file_path: &str) -> Result<ParseResult> {
    let lines: Vec<&str> = source.lines().collect();
    let loc = lines.len() as u32;
    let hash = content_hash(source);

    let file_entity = make_entity(
        "file",
        file_path.rsplit('/').next().unwrap_or(file_path),
        file_path,
        file_path,
        1,
        loc,
        serde_json::json!({
            "path": file_path,
            "language": "unknown",
            "loc": loc,
            "hash": hash,
        }),
        Some(&hash),
    );

    Ok(ParseResult {
        entities: vec![file_entity],
        relationships: Vec::new(),
    })
}
