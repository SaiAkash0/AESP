pub mod entity;
mod relationship;
mod annotation;
mod query;
mod traversal;

pub use entity::*;
pub use relationship::*;
pub use annotation::*;
pub use query::*;
pub use traversal::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub entity_type: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub properties: serde_json::Value,
    pub source_type: String,
    pub verification_status: String,
    pub confidence: f64,
    pub memory_scope: String,
    pub content_hash: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: String,
    pub properties: serde_json::Value,
    pub source_type: String,
    pub verification_status: String,
    pub confidence: f64,
    pub weight: f64,
    pub memory_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub entity_id: String,
    pub annotation_type: String,
    pub content: String,
    pub author: String,
    pub tags: Vec<String>,
    pub resolved: bool,
    pub source_type: String,
    pub verification_status: String,
    pub confidence: f64,
    pub memory_scope: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub id: String,
    pub entity_type: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub verification_status: String,
    pub confidence: f64,
    pub signature: Option<String>,
    pub relevance_score: f64,
    pub relationships: Vec<RelationshipInfo>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipInfo {
    pub relationship_type: String,
    pub direction: String,
    pub target_qualified_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatus {
    pub total_entities: u64,
    pub total_relationships: u64,
    pub total_annotations: u64,
    pub total_decisions: u64,
    pub total_events: u64,
    pub active_constraints: u64,
    pub entities_by_type: Vec<(String, u64)>,
    pub entities_by_verification: Vec<(String, u64)>,
}
