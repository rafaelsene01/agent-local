use super::embedding::EMBEDDING_DIM;
use arrow_array::types::Float32Type;
use arrow_array::{
    Array, FixedSizeListArray, Int32Array, RecordBatch, RecordBatchIterator, RecordBatchReader,
    StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, Table};
use std::path::Path;
use std::sync::Arc;

const TABLE_NAME: &str = "chunks";

/// One logical table for every namespace, filtered by column: creating and
/// dropping a physical table per chat would mean managing table lifecycles
/// for something LanceDB already indexes well.
pub const GLOBAL_NAMESPACE: &str = "global";

pub fn chat_namespace(chat_id: &str) -> String {
    format!("chat:{chat_id}")
}

#[derive(Debug)]
pub enum StoreError {
    Backend(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Backend(msg) => write!(f, "falha no banco vetorial: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}

#[derive(Debug, Clone)]
pub struct EmbeddedChunk {
    pub id: String,
    pub text: String,
    pub vector: Vec<f32>,
    pub chunk_index: i32,
}

#[derive(Debug, Clone)]
pub struct RetrievedChunk {
    pub doc_id: String,
    pub text: String,
    pub chunk_index: i32,
    pub distance: f32,
}

pub struct VectorStore {
    connection: Connection,
}

fn schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("namespace", DataType::Utf8, false),
        Field::new("doc_id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("chunk_index", DataType::Int32, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM as i32,
            ),
            true,
        ),
    ]))
}

/// SQL string literals are single-quoted, so an embedded quote would end the
/// literal early. Ids are UUIDs today, but the filter must not depend on that.
fn escape_sql(value: &str) -> String {
    value.replace('\'', "''")
}

impl VectorStore {
    pub async fn open(vectors_dir: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(vectors_dir).map_err(backend)?;
        let uri = vectors_dir.to_string_lossy().to_string();
        let connection = lancedb::connect(&uri).execute().await.map_err(backend)?;
        Ok(VectorStore { connection })
    }

    /// The table is created on first write, not at startup, so an app that
    /// never indexes anything never creates an empty store.
    async fn table(&self) -> Result<Option<Table>, StoreError> {
        let names = self.connection.table_names().execute().await.map_err(backend)?;
        if !names.iter().any(|n| n == TABLE_NAME) {
            return Ok(None);
        }
        self.connection
            .open_table(TABLE_NAME)
            .execute()
            .await
            .map(Some)
            .map_err(backend)
    }

    pub async fn upsert(
        &self,
        namespace: &str,
        doc_id: &str,
        chunks: Vec<EmbeddedChunk>,
    ) -> Result<(), StoreError> {
        if chunks.is_empty() {
            return Ok(());
        }
        // Re-indexing a document replaces it instead of duplicating chunks.
        self.delete_by_doc(namespace, doc_id).await?;

        let schema = schema();
        let ids = StringArray::from(chunks.iter().map(|c| c.id.clone()).collect::<Vec<_>>());
        let namespaces = StringArray::from(vec![namespace.to_string(); chunks.len()]);
        let doc_ids = StringArray::from(vec![doc_id.to_string(); chunks.len()]);
        let texts = StringArray::from(chunks.iter().map(|c| c.text.clone()).collect::<Vec<_>>());
        let indexes = Int32Array::from(chunks.iter().map(|c| c.chunk_index).collect::<Vec<_>>());
        let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
            chunks
                .iter()
                .map(|c| Some(c.vector.iter().map(|v| Some(*v)).collect::<Vec<_>>()))
                .collect::<Vec<_>>(),
            EMBEDDING_DIM as i32,
        );

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(ids),
                Arc::new(namespaces),
                Arc::new(doc_ids),
                Arc::new(texts),
                Arc::new(indexes),
                Arc::new(vectors),
            ],
        )
        .map_err(backend)?;

        // lancedb takes a boxed RecordBatchReader, not the iterator itself.
        let batches = Box::new(RecordBatchIterator::new(vec![Ok(batch)], schema))
            as Box<dyn RecordBatchReader + Send>;

        match self.table().await? {
            Some(table) => {
                table.add(batches).execute().await.map_err(backend)?;
            }
            None => {
                self.connection
                    .create_table(TABLE_NAME, batches)
                    .execute()
                    .await
                    .map_err(backend)?;
            }
        }
        Ok(())
    }

    /// Returns an empty list when nothing was ever indexed — an empty
    /// knowledge base is a normal state, not an error (DOC-11).
    pub async fn search(
        &self,
        namespace: &str,
        query_vec: &[f32],
        top_k: usize,
    ) -> Result<Vec<RetrievedChunk>, StoreError> {
        let Some(table) = self.table().await? else {
            return Ok(Vec::new());
        };

        let batches = table
            .query()
            .nearest_to(query_vec)
            .map_err(backend)?
            .only_if(format!("namespace = '{}'", escape_sql(namespace)))
            .limit(top_k)
            .execute()
            .await
            .map_err(backend)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(backend)?;

        let mut out = Vec::new();
        for batch in batches {
            out.extend(rows_from_batch(&batch)?);
        }
        Ok(out)
    }

    /// Fetches one chunk by its position in a document.
    ///
    /// Used to append the passage that immediately follows a hit: when someone
    /// asks to continue a text, the continuation is in the next chunk far more
    /// often than in whatever else the query happens to be close to.
    pub async fn chunk_at(
        &self,
        namespace: &str,
        doc_id: &str,
        chunk_index: i32,
    ) -> Result<Option<RetrievedChunk>, StoreError> {
        let Some(table) = self.table().await? else {
            return Ok(None);
        };

        let batches = table
            .query()
            .only_if(format!(
                "namespace = '{}' AND doc_id = '{}' AND chunk_index = {}",
                escape_sql(namespace),
                escape_sql(doc_id),
                chunk_index
            ))
            .limit(1)
            .execute()
            .await
            .map_err(backend)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(backend)?;

        for batch in batches {
            if let Some(chunk) = rows_from_batch(&batch)?.into_iter().next() {
                return Ok(Some(chunk));
            }
        }
        Ok(None)
    }

    pub async fn delete_by_doc(&self, namespace: &str, doc_id: &str) -> Result<(), StoreError> {
        let Some(table) = self.table().await? else {
            return Ok(());
        };
        table
            .delete(&format!(
                "namespace = '{}' AND doc_id = '{}'",
                escape_sql(namespace),
                escape_sql(doc_id)
            ))
            .await
            .map(|_| ())
            .map_err(backend)
    }

    pub async fn delete_namespace(&self, namespace: &str) -> Result<(), StoreError> {
        let Some(table) = self.table().await? else {
            return Ok(());
        };
        table
            .delete(&format!("namespace = '{}'", escape_sql(namespace)))
            .await
            .map(|_| ())
            .map_err(backend)
    }
}

/// `_distance` is only present on vector searches; a plain filtered read has no
/// score, and NaN says exactly that instead of pretending it is a perfect match.
fn rows_from_batch(batch: &RecordBatch) -> Result<Vec<RetrievedChunk>, StoreError> {
    let doc_ids = column_as_strings(batch, "doc_id")?;
    let texts = column_as_strings(batch, "text")?;
    let indexes = batch
        .column_by_name("chunk_index")
        .and_then(|c| c.as_any().downcast_ref::<Int32Array>())
        .ok_or_else(|| StoreError::Backend("coluna chunk_index ausente".to_string()))?;
    let distances = batch
        .column_by_name("_distance")
        .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());

    Ok((0..batch.num_rows())
        .map(|row| RetrievedChunk {
            doc_id: doc_ids.value(row).to_string(),
            text: texts.value(row).to_string(),
            chunk_index: indexes.value(row),
            distance: distances.map(|d| d.value(row)).unwrap_or(f32::NAN),
        })
        .collect())
}

fn column_as_strings<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, StoreError> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| StoreError::Backend(format!("coluna {name} ausente no resultado")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_namespaces_are_prefixed_and_never_collide_with_global() {
        assert_eq!(chat_namespace("abc"), "chat:abc");
        assert_ne!(chat_namespace("global"), GLOBAL_NAMESPACE);
    }

    #[test]
    fn quotes_in_filter_values_are_escaped() {
        assert_eq!(escape_sql("o'brien"), "o''brien");
    }

    fn fake_chunk(id: &str, text: &str, seed: f32) -> EmbeddedChunk {
        EmbeddedChunk {
            id: id.to_string(),
            text: text.to_string(),
            vector: vec![seed; EMBEDDING_DIM],
            chunk_index: 0,
        }
    }

    /// Hits a real LanceDB on disk, so it is excluded from the default run.
    /// Run with: `cargo test store -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "writes a real LanceDB table to a temp folder"]
    async fn namespaces_are_isolated_and_deletes_remove_only_their_rows() {
        let dir = std::env::temp_dir().join(format!("localmind-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = VectorStore::open(&dir).await.unwrap();

        store
            .upsert("global", "doc-a", vec![fake_chunk("1", "conteúdo global", 0.1)])
            .await
            .unwrap();
        store
            .upsert(
                &chat_namespace("chat-1"),
                "att-b",
                vec![fake_chunk("2", "anexo do chat", 0.9)],
            )
            .await
            .unwrap();

        let query = vec![0.1f32; EMBEDDING_DIM];
        let global_hits = store.search("global", &query, 10).await.unwrap();
        assert_eq!(global_hits.len(), 1, "one namespace must not see the other");
        assert_eq!(global_hits[0].doc_id, "doc-a");

        let chat_hits = store
            .search(&chat_namespace("chat-1"), &query, 10)
            .await
            .unwrap();
        assert_eq!(chat_hits.len(), 1);
        assert_eq!(chat_hits[0].text, "anexo do chat");

        // Deleting the chat's namespace must leave the global one untouched.
        store.delete_namespace(&chat_namespace("chat-1")).await.unwrap();
        assert!(store
            .search(&chat_namespace("chat-1"), &query, 10)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(store.search("global", &query, 10).await.unwrap().len(), 1);

        store.delete_by_doc("global", "doc-a").await.unwrap();
        assert!(store.search("global", &query, 10).await.unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Searching before anything was ever indexed is a normal state (DOC-11).
    #[tokio::test]
    #[ignore = "touches the filesystem"]
    async fn searching_an_empty_store_returns_nothing_instead_of_failing() {
        let dir = std::env::temp_dir().join(format!("localmind-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = VectorStore::open(&dir).await.unwrap();

        let hits = store
            .search("global", &vec![0.0; EMBEDDING_DIM], 5)
            .await
            .unwrap();

        assert!(hits.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
