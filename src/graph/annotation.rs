use anyhow::Result;
use crate::storage::Storage;
use crate::storage::queries;
use super::Annotation;

pub fn insert_annotation(storage: &Storage, annotation: &Annotation) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            queries::INSERT_ANNOTATION,
            rusqlite::params![
                annotation.id,
                annotation.entity_id,
                annotation.annotation_type,
                annotation.content,
                annotation.author,
                serde_json::to_string(&annotation.tags)?,
                annotation.source_type,
                annotation.verification_status,
                annotation.confidence,
                annotation.memory_scope,
                Option::<String>::None, // session_id
            ],
        )?;
        Ok(())
    })
}

pub fn get_annotations_for_entity(storage: &Storage, entity_id: &str) -> Result<Vec<Annotation>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::GET_ANNOTATIONS_FOR_ENTITY)?;
        let mut rows = stmt.query(rusqlite::params![entity_id])?;
        let mut annotations = Vec::new();
        while let Some(row) = rows.next()? {
            let tags_str: String = row.get(5)?;
            let tags: Vec<String> =
                serde_json::from_str(&tags_str).unwrap_or_default();
            let resolved_int: i32 = row.get(9)?;

            annotations.push(Annotation {
                id: row.get(0)?,
                entity_id: row.get(1)?,
                annotation_type: row.get(2)?,
                content: row.get(3)?,
                author: row.get(4)?,
                tags,
                resolved: resolved_int != 0,
                source_type: row.get(6)?,
                verification_status: row.get(7)?,
                confidence: row.get(8)?,
                memory_scope: row.get(10)?,
                created_at: row.get(11)?,
            });
        }
        Ok(annotations)
    })
}
