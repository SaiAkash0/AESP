pub mod languages;
pub mod treesitter;

use anyhow::Result;
use std::path::Path;
use crate::graph::{Entity, Relationship};

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

pub fn parse_file(file_path: &Path, project_root: &Path) -> Result<ParseResult> {
    let content = std::fs::read_to_string(file_path)?;
    let relative_path = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('\\', "/");

    let lang = languages::detect_language(file_path);

    match lang {
        Some(languages::Language::TypeScript) | Some(languages::Language::JavaScript) => {
            treesitter::parse_typescript(&content, &relative_path)
        }
        Some(languages::Language::Python) => {
            treesitter::parse_python(&content, &relative_path)
        }
        _ => {
            treesitter::parse_generic(&content, &relative_path)
        }
    }
}
