use rusqlite::params;
use rusqlite::OptionalExtension;

use crate::db::Database;
use crate::errors::Result;
use crate::types::Node;

/// Compute cosine similarity between two vectors.
///
/// Returns 0.0 if either vector has zero magnitude.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

/// Store an embedding vector in the database.
///
/// The embedding is serialized as a little-endian byte blob. If a vector
/// already exists for `node_id`, it is replaced.
pub fn store_vector(db: &Database, node_id: &str, embedding: &[f32], model: &str) -> Result<()> {
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    db.conn().execute(
        "INSERT OR REPLACE INTO vectors (node_id, embedding, model, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![node_id, bytes, model, now],
    )?;
    Ok(())
}

/// Retrieve an embedding vector from the database.
///
/// Returns `None` if no vector is stored for the given `node_id`.
/// The blob is deserialized from little-endian f32 values.
pub fn get_vector(db: &Database, node_id: &str) -> Result<Option<Vec<f32>>> {
    let result: Option<Vec<u8>> = db
        .conn()
        .query_row(
            "SELECT embedding FROM vectors WHERE node_id = ?1",
            params![node_id],
            |row| row.get(0),
        )
        .optional()?;

    match result {
        Some(bytes) => {
            let floats = bytes_to_f32s(&bytes);
            Ok(Some(floats))
        }
        None => Ok(None),
    }
}

/// Brute-force cosine similarity search across all stored vectors.
///
/// Loads every vector from the database, computes cosine similarity against
/// `query`, and returns the top `limit` results sorted by descending similarity.
pub fn brute_force_search(
    db: &Database,
    query: &[f32],
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT node_id, embedding FROM vectors")?;

    let rows = stmt.query_map([], |row| {
        let node_id: String = row.get(0)?;
        let bytes: Vec<u8> = row.get(1)?;
        Ok((node_id, bytes))
    })?;

    let mut scored: Vec<(String, f32)> = Vec::new();
    for row in rows {
        let (node_id, bytes) = row?;
        let embedding = bytes_to_f32s(&bytes);
        let score = cosine_similarity(query, &embedding);
        scored.push((node_id, score));
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    Ok(scored)
}

/// Get the count of stored vectors.
pub fn vector_count(db: &Database) -> Result<usize> {
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM vectors", [], |row| row.get(0))?;
    Ok(count as usize)
}

/// Delete a vector for a node.
pub fn delete_vector(db: &Database, node_id: &str) -> Result<()> {
    db.conn()
        .execute("DELETE FROM vectors WHERE node_id = ?1", params![node_id])?;
    Ok(())
}

/// Clear all vectors.
pub fn clear_vectors(db: &Database) -> Result<()> {
    db.conn().execute("DELETE FROM vectors", [])?;
    Ok(())
}

/// Create searchable text from a Node for embedding.
///
/// Formats the node's key fields into a human-readable string suitable for
/// generating a text embedding. Only fields that are `Some` are included.
pub fn create_node_text(node: &Node) -> String {
    let mut parts = Vec::new();
    parts.push(format!("kind: {}", node.kind.as_str()));
    parts.push(format!("name: {}", node.name));
    parts.push(format!("qualified_name: {}", node.qualified_name));
    parts.push(format!("file: {}", node.file_path));
    if let Some(ref sig) = node.signature {
        parts.push(format!("signature: {sig}"));
    }
    if let Some(ref doc) = node.docstring {
        parts.push(format!("docstring: {doc}"));
    }
    parts.join("\n")
}

/// Convert a byte slice to a vector of f32 values (little-endian).
fn bytes_to_f32s(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(arr)
        })
        .collect()
}
