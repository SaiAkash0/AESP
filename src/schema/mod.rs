mod types;
mod validator;

pub use types::*;
pub use validator::*;

use anyhow::Result;

pub struct SchemaRegistry {
    schemas: Vec<Schema>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        let code_schema = Schema {
            name: "code".to_string(),
            version: "1.0.0".to_string(),
            entity_types: vec![
                "project".into(),
                "module".into(),
                "file".into(),
                "function".into(),
                "class".into(),
                "type_definition".into(),
                "variable".into(),
                "dependency".into(),
                "config_file".into(),
                "test_file".into(),
            ],
            relationship_types: vec![
                "contains".into(),
                "calls".into(),
                "imports".into(),
                "extends".into(),
                "implements".into(),
                "type_references".into(),
                "tests".into(),
                "reads_config".into(),
            ],
        };

        SchemaRegistry {
            schemas: vec![code_schema],
        }
    }

    pub fn get_schema(&self, name: &str) -> Result<&Schema> {
        self.schemas
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Schema '{}' not found", name))
    }

    pub fn list_schemas(&self) -> &[Schema] {
        &self.schemas
    }
}
