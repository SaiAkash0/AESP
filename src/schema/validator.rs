use anyhow::Result;
use super::Schema;

pub fn validate_entity_type(schema: &Schema, entity_type: &str) -> Result<()> {
    if schema.entity_types.contains(&entity_type.to_string()) {
        Ok(())
    } else {
        anyhow::bail!(
            "Entity type '{}' not found in schema '{}'. Valid types: {:?}",
            entity_type,
            schema.name,
            schema.entity_types
        )
    }
}

pub fn validate_relationship_type(schema: &Schema, rel_type: &str) -> Result<()> {
    if schema.relationship_types.contains(&rel_type.to_string()) {
        Ok(())
    } else {
        anyhow::bail!(
            "Relationship type '{}' not found in schema '{}'. Valid types: {:?}",
            rel_type,
            schema.name,
            schema.relationship_types
        )
    }
}
