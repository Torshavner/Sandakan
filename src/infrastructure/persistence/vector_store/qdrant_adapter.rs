use async_trait::async_trait;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder, Distance,
    FieldType, PointId, PointStruct, PointsIdsList, SearchPointsBuilder, UpsertPointsBuilder,
    VectorParamsBuilder, VectorsConfig,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::application::ports::{
    CollectionConfig, DistanceMetric, PayloadFieldType, SearchResult, VectorStore, VectorStoreError,
};
use crate::domain::{Chunk, ChunkId, DocumentId, Embedding};

pub struct QdrantAdapter {
    client: Arc<Qdrant>,
    collection_name: String,
}

impl QdrantAdapter {
    pub async fn new(url: &str, collection_name: String) -> Result<Self, VectorStoreError> {
        let client = Qdrant::from_url(url)
            .build()
            .map_err(|e| VectorStoreError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            client: Arc::new(client),
            collection_name,
        })
    }

    pub fn with_client(client: Arc<Qdrant>, collection_name: String) -> Self {
        Self {
            client,
            collection_name,
        }
    }

    fn map_distance_metric(metric: &DistanceMetric) -> Distance {
        match metric {
            DistanceMetric::Cosine => Distance::Cosine,
            DistanceMetric::Euclidean => Distance::Euclid,
            DistanceMetric::DotProduct => Distance::Dot,
        }
    }

    fn map_field_type(field_type: &PayloadFieldType) -> FieldType {
        match field_type {
            PayloadFieldType::Keyword => FieldType::Keyword,
            PayloadFieldType::Integer => FieldType::Integer,
            PayloadFieldType::Float => FieldType::Float,
            PayloadFieldType::Text => FieldType::Text,
        }
    }
}

#[async_trait]
impl VectorStore for QdrantAdapter {
    #[instrument(skip(self, config), fields(collection = %self.collection_name))]
    async fn create_collection(&self, config: &CollectionConfig) -> Result<bool, VectorStoreError> {
        if self.collection_exists().await? {
            info!(collection = %self.collection_name, "collection already exists");
            return Ok(false);
        }

        let vectors_config = VectorsConfig::from(VectorParamsBuilder::new(
            config.vector_dimensions,
            Self::map_distance_metric(&config.distance_metric),
        ));

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection_name).vectors_config(vectors_config),
            )
            .await
            .map_err(|e| VectorStoreError::CollectionCreationFailed(e.to_string()))?;

        info!(collection = %self.collection_name, "collection_created");

        for index in &config.payload_indexes {
            self.client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    &self.collection_name,
                    &index.field_name,
                    Self::map_field_type(&index.field_type),
                ))
                .await
                .map_err(|e| VectorStoreError::PayloadIndexFailed(e.to_string()))?;

            info!(
                collection = %self.collection_name,
                field = %index.field_name,
                "payload_index_applied"
            );
        }

        Ok(true)
    }

    #[instrument(skip(self), fields(collection = %self.collection_name))]
    async fn collection_exists(&self) -> Result<bool, VectorStoreError> {
        self.client
            .collection_exists(&self.collection_name)
            .await
            .map_err(|e| VectorStoreError::ConnectionFailed(e.to_string()))
    }

    #[instrument(skip(self), fields(collection = %self.collection_name))]
    async fn get_collection_vector_size(&self) -> Result<Option<u64>, VectorStoreError> {
        if !self.collection_exists().await? {
            return Ok(None);
        }

        let collection_info = self
            .client
            .collection_info(&self.collection_name)
            .await
            .map_err(|e| VectorStoreError::ConnectionFailed(e.to_string()))?;

        let vector_size = collection_info
            .result
            .and_then(|result| result.config)
            .and_then(|config| config.params)
            .and_then(|params| params.vectors_config)
            .and_then(|vectors_config| match vectors_config.config {
                Some(qdrant_client::qdrant::vectors_config::Config::Params(params)) => {
                    Some(params.size)
                }
                _ => None,
            });

        Ok(vector_size)
    }

    #[instrument(skip(self), fields(collection = %self.collection_name))]
    async fn delete_collection(&self) -> Result<(), VectorStoreError> {
        if !self.collection_exists().await? {
            return Ok(());
        }

        self.client
            .delete_collection(&self.collection_name)
            .await
            .map_err(|e| VectorStoreError::CollectionDeletionFailed(e.to_string()))?;

        info!(collection = %self.collection_name, "collection_deleted");
        Ok(())
    }

    #[instrument(skip(self, chunks, embeddings), fields(collection = %self.collection_name, count = chunks.len()))]
    async fn upsert(
        &self,
        chunks: &[Chunk],
        embeddings: &[Embedding],
    ) -> Result<(), VectorStoreError> {
        if chunks.len() != embeddings.len() {
            return Err(VectorStoreError::UpsertFailed(
                "chunks and embeddings count mismatch".to_string(),
            ));
        }

        let points: Vec<PointStruct> = chunks
            .iter()
            .zip(embeddings.iter())
            .map(|(chunk, embedding)| {
                let mut payload: HashMap<String, serde_json::Value> = HashMap::new();
                payload.insert(
                    "document_id".to_string(),
                    serde_json::Value::String(chunk.document_id.as_uuid().to_string()),
                );
                payload.insert(
                    "text".to_string(),
                    serde_json::Value::String(chunk.text.clone()),
                );
                payload.insert(
                    "page".to_string(),
                    chunk
                        .page
                        .map(|p| serde_json::Value::Number(p.into()))
                        .unwrap_or(serde_json::Value::Null),
                );
                payload.insert(
                    "offset".to_string(),
                    serde_json::Value::Number((chunk.offset as u64).into()),
                );

                PointStruct::new(
                    PointId::from(chunk.id.as_uuid().to_string()),
                    embedding.values.clone(),
                    payload,
                )
            })
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await
            .map_err(|e| VectorStoreError::UpsertFailed(e.to_string()))?;

        info!(collection = %self.collection_name, count = chunks.len(), "points_upserted");
        Ok(())
    }

    #[instrument(skip(self, embedding), fields(collection = %self.collection_name, top_k = top_k))]
    async fn search(
        &self,
        embedding: &Embedding,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        let search_result = self
            .client
            .search_points(
                SearchPointsBuilder::new(
                    &self.collection_name,
                    embedding.values.clone(),
                    top_k as u64,
                )
                .with_payload(true),
            )
            .await
            .map_err(|e| VectorStoreError::SearchFailed(e.to_string()))?;

        let results: Vec<SearchResult> = search_result
            .result
            .into_iter()
            .filter_map(|point| {
                let payload = point.payload;

                let document_id_str = payload.get("document_id")?.as_str()?;
                let document_id = Uuid::parse_str(document_id_str).ok()?;

                let chunk_id_str = point.id?.point_id_options?;
                let chunk_id = match chunk_id_str {
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid) => {
                        Uuid::parse_str(&uuid).ok()?
                    }
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(_) => return None,
                };

                let text = payload.get("text")?.as_str()?.to_string();
                let page = payload
                    .get("page")
                    .and_then(|v| v.as_integer())
                    .map(|v| v as u32);
                let offset = payload.get("offset")?.as_integer()? as usize;

                let chunk = Chunk {
                    id: ChunkId::from_uuid(chunk_id),
                    text,
                    document_id: DocumentId::from_uuid(document_id),
                    page,
                    offset,
                };

                Some(SearchResult {
                    chunk,
                    score: point.score,
                })
            })
            .collect();

        Ok(results)
    }

    #[instrument(skip(self, chunk_ids), fields(collection = %self.collection_name, count = chunk_ids.len()))]
    async fn delete(&self, chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        let point_ids: Vec<PointId> = chunk_ids
            .iter()
            .map(|id| PointId::from(id.as_uuid().to_string()))
            .collect();

        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection_name)
                    .points(PointsIdsList { ids: point_ids }),
            )
            .await
            .map_err(|e| VectorStoreError::DeleteFailed(e.to_string()))?;

        info!(collection = %self.collection_name, count = chunk_ids.len(), "points_deleted");
        Ok(())
    }
}
