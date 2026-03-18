use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub version: String,
    pub entity_types: Vec<String>,
    pub relationship_types: Vec<String>,
}
