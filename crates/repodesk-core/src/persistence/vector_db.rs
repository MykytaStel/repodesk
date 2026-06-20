use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::persistence::db::init_db;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRecord {
    pub id: i64,
    pub project: String,
    pub file_path: String,
    pub chunk_index: i64,
    pub content: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub chunk_index: i64,
    pub content: String,
    pub score: f32,
}

fn vec_to_blob(vec: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(vec.len() * 4);
    for &val in vec {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    let mut vec = Vec::with_capacity(blob.len() / 4);
    for chunk in blob.chunks_exact(4) {
        let val = f32::from_le_bytes(chunk.try_into().unwrap());
        vec.push(val);
    }
    vec
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot = dot_product(a, b);
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub fn insert_embedding(
    project: &str,
    file_path: &str,
    chunk_index: i64,
    content: &str,
    embedding: &[f32],
) -> RepoDeskResult<()> {
    let conn = init_db()?;
    let blob = vec_to_blob(embedding);

    conn.execute(
        "INSERT INTO project_embeddings (project, file_path, chunk_index, content, embedding_blob)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (project, file_path, chunk_index, content, blob),
    )
    .map_err(|e| RepoDeskError::Database(format!("Failed to insert embedding: {}", e)))?;

    Ok(())
}

pub fn delete_embeddings_for_file(project: &str, file_path: &str) -> RepoDeskResult<()> {
    let conn = init_db()?;
    conn.execute(
        "DELETE FROM project_embeddings WHERE project = ?1 AND file_path = ?2",
        (project, file_path),
    )
    .map_err(|e| RepoDeskError::Database(format!("Failed to delete file embeddings: {}", e)))?;

    Ok(())
}

pub fn search_similar(
    project: &str,
    query_embedding: &[f32],
    limit: usize,
) -> RepoDeskResult<Vec<SearchResult>> {
    let conn = init_db()?;
    let mut stmt = conn
        .prepare("SELECT file_path, chunk_index, content, embedding_blob FROM project_embeddings WHERE project = ?1")
        .map_err(|e| RepoDeskError::Database(format!("Failed to prepare search query: {}", e)))?;

    let mut rows = stmt
        .query([project])
        .map_err(|e| RepoDeskError::Database(format!("Failed to execute search query: {}", e)))?;

    let mut results = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| RepoDeskError::Database(format!("Failed to fetch search row: {}", e)))?
    {
        let file_path: String = row.get::<_, String>(0).unwrap();
        let chunk_index: i64 = row.get::<_, i64>(1).unwrap();
        let content: String = row.get::<_, String>(2).unwrap();
        let blob: Vec<u8> = row.get::<_, Vec<u8>>(3).unwrap();

        let embedding = blob_to_vec(&blob);
        let score = cosine_similarity(query_embedding, &embedding);
        
        results.push(SearchResult {
            file_path,
            chunk_index,
            content,
            score,
        });
    }

    // Sort descending by score
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    Ok(results)
}
