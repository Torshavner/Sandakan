// @AI-BYPASS-LENGTH
use async_trait::async_trait;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder, Distance,
    FieldType, Fusion, NamedVectors, PointId, PointStruct, PointsIdsList, PrefetchQueryBuilder,
    Query, QueryPointsBuilder, ScoredPoint, SearchPointsBuilder, SparseVectorParamsBuilder,
    UpsertPointsBuilder, Vector, VectorInput, VectorParamsBuilder, VectorsConfig,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::application::ports::{
    CollectionConfig, DistanceMetric, PayloadFieldType, SearchResult, VectorStore, VectorStoreError,
};
use crate::domain::{
    Chunk, ChunkId, ContentType, DocumentId, DocumentMetadata, Embedding, SparseEmbedding,
};

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

    fn build_payload(chunk: &Chunk) -> HashMap<String, serde_json::Value> {
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

        if let Some(meta) = &chunk.metadata {
            payload.insert(
                "title".to_string(),
                serde_json::Value::String(meta.title.clone()),
            );
            payload.insert(
                "content_type".to_string(),
                serde_json::Value::String(meta.content_type.as_mime().to_string()),
            );
            if let Some(url) = &meta.source_url {
                payload.insert(
                    "source_url".to_string(),
                    serde_json::Value::String(url.clone()),
                );
            }
        }

        if let Some(start_time) = chunk.start_time {
            payload.insert(
                "start_time".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(start_time as f64)
                        .unwrap_or(serde_json::Number::from(0)),
                ),
            );
        }

        payload
    }

    fn map_scored_point(point: ScoredPoint) -> Option<SearchResult> {
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

        let metadata = payload.get("title").and_then(|v| v.as_str()).map(|title| {
            let content_type = payload
                .get("content_type")
                .and_then(|v| v.as_str())
                .and_then(|s| ContentType::from_mime(s.as_str()))
                .unwrap_or(ContentType::Text);
            let source_url = payload
                .get("source_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Arc::new(DocumentMetadata {
                title: title.to_string(),
                content_type,
                source_url,
            })
        });

        let start_time = payload
            .get("start_time")
            .and_then(|v| v.as_double())
            .map(|v| v as f32);

        let chunk = Chunk {
            id: ChunkId::from_uuid(chunk_id),
            text,
            document_id: DocumentId::from_uuid(document_id),
            page,
            offset,
            metadata,
            start_time,
        };

        if point.score.is_nan() {
            tracing::warn!(chunk_id = %chunk_id, "Qdrant returned NaN score — skipping result");
            return None;
        }

        Some(SearchResult {
            chunk,
            score: point.score,
        })
    }

    fn log_nan_filtered(total: usize, valid: usize) {
        if valid < total {
            tracing::warn!(
                total,
                valid,
                nan_filtered = total - valid,
                "Search results contained NaN scores — re-index the collection to fix"
            );
        }
    }

    fn log_bad_embeddings(embeddings: &[Embedding]) {
        let bad_count = embeddings
            .iter()
            .filter(|e| {
                e.values.iter().any(|v| !v.is_finite()) || e.values.iter().all(|v| *v == 0.0)
            })
            .count();
        if bad_count > 0 {
            tracing::error!(
                bad_count,
                total = embeddings.len(),
                "Upserting invalid embeddings (NaN/Inf/zero) — these will cause NaN search scores"
            );
        }
    }

    fn log_bad_query_embedding(embedding: &Embedding) {
        let has_nan = embedding.values.iter().any(|v| !v.is_finite());
        let is_zero = embedding.values.iter().all(|v| *v == 0.0);
        if has_nan || is_zero {
            tracing::error!(
                has_nan,
                is_zero,
                dims = embedding.values.len(),
                "Query embedding is invalid — embedder producing bad vectors"
            );
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

        let builder = if config.hybrid {
            use qdrant_client::qdrant::{SparseVectorsConfigBuilder, VectorsConfigBuilder};

            let mut vectors = VectorsConfigBuilder::default();
            vectors.add_named_vector_params(
                "dense",
                VectorParamsBuilder::new(
                    config.vector_dimensions,
                    Self::map_distance_metric(&config.distance_metric),
                ),
            );

            let mut sparse = SparseVectorsConfigBuilder::default();
            sparse.add_named_vector_params("sparse", SparseVectorParamsBuilder::default());

            CreateCollectionBuilder::new(&self.collection_name)
                .vectors_config(vectors)
                .sparse_vectors_config(sparse)
        } else {
            let vectors_config = VectorsConfig::from(VectorParamsBuilder::new(
                config.vector_dimensions,
                Self::map_distance_metric(&config.distance_metric),
            ));
            CreateCollectionBuilder::new(&self.collection_name).vectors_config(vectors_config)
        };

        self.client
            .create_collection(builder)
            .await
            .map_err(|e| VectorStoreError::CollectionCreationFailed(e.to_string()))?;

        info!(collection = %self.collection_name, hybrid = config.hybrid, "collection_created");

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
                Some(qdrant_client::qdrant::vectors_config::Config::ParamsMap(map)) => {
                    map.map.get("dense").map(|p| p.size)
                }
                _ => None,
            });

        Ok(vector_size)
    }

    #[instrument(skip(self), fields(collection = %self.collection_name))]
    async fn is_hybrid_collection(&self) -> Result<bool, VectorStoreError> {
        if !self.collection_exists().await? {
            return Ok(false);
        }

        let collection_info = self
            .client
            .collection_info(&self.collection_name)
            .await
            .map_err(|e| VectorStoreError::ConnectionFailed(e.to_string()))?;

        let is_hybrid = collection_info
            .result
            .and_then(|r| r.config)
            .and_then(|c| c.params)
            .and_then(|p| p.vectors_config)
            .map(|vc| {
                matches!(
                    vc.config,
                    Some(qdrant_client::qdrant::vectors_config::Config::ParamsMap(_))
                )
            })
            .unwrap_or(false);

        Ok(is_hybrid)
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

        Self::log_bad_embeddings(embeddings);

        let points: Vec<PointStruct> = chunks
            .iter()
            .zip(embeddings.iter())
            .map(|(chunk, embedding)| {
                let payload = Self::build_payload(chunk);
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

    #[instrument(skip(self, chunks, dense, sparse), fields(collection = %self.collection_name, count = chunks.len()))]
    async fn upsert_hybrid(
        &self,
        chunks: &[Chunk],
        dense: &[Embedding],
        sparse: &[SparseEmbedding],
    ) -> Result<(), VectorStoreError> {
        if chunks.len() != dense.len() || chunks.len() != sparse.len() {
            return Err(VectorStoreError::UpsertFailed(
                "chunks, dense, and sparse embeddings count mismatch".to_string(),
            ));
        }

        Self::log_bad_embeddings(dense);

        let points: Vec<PointStruct> = chunks
            .iter()
            .zip(dense.iter())
            .zip(sparse.iter())
            .map(|((chunk, d_emb), s_emb)| {
                let payload = Self::build_payload(chunk);

                let named = NamedVectors::default()
                    .add_vector("dense", Vector::new_dense(d_emb.values.clone()))
                    .add_vector(
                        "sparse",
                        Vector::new_sparse(s_emb.indices.clone(), s_emb.values.clone()),
                    );

                PointStruct::new(
                    PointId::from(chunk.id.as_uuid().to_string()),
                    named,
                    payload,
                )
            })
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await
            .map_err(|e| VectorStoreError::UpsertFailed(e.to_string()))?;

        info!(collection = %self.collection_name, count = chunks.len(), "hybrid_points_upserted");
        Ok(())
    }

    #[instrument(skip(self, embedding), fields(collection = %self.collection_name, top_k = top_k))]
    async fn search(
        &self,
        embedding: &Embedding,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Self::log_bad_query_embedding(embedding);

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

        let total = search_result.result.len();
        let results: Vec<SearchResult> = search_result
            .result
            .into_iter()
            .filter_map(Self::map_scored_point)
            .collect();

        Self::log_nan_filtered(total, results.len());
        Ok(results)
    }

    #[instrument(skip(self, dense, sparse), fields(collection = %self.collection_name, top_k = top_k))]
    async fn search_hybrid(
        &self,
        dense: &Embedding,
        sparse: &SparseEmbedding,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Self::log_bad_query_embedding(dense);

        let prefetch_limit = (top_k as u64).saturating_mul(2).max(10);

        let dense_prefetch = PrefetchQueryBuilder::default()
            .query(Query::new_nearest(VectorInput::new_dense(
                dense.values.clone(),
            )))
            .using("dense")
            .limit(prefetch_limit);

        let sparse_prefetch = PrefetchQueryBuilder::default()
            .query(Query::new_nearest(VectorInput::new_sparse(
                sparse.indices.clone(),
                sparse.values.clone(),
            )))
            .using("sparse")
            .limit(prefetch_limit);

        let response = self
            .client
            .query(
                QueryPointsBuilder::new(&self.collection_name)
                    .add_prefetch(dense_prefetch)
                    .add_prefetch(sparse_prefetch)
                    .query(Query::new_fusion(Fusion::Rrf))
                    .limit(top_k as u64)
                    .with_payload(true),
            )
            .await
            .map_err(|e| VectorStoreError::SearchFailed(e.to_string()))?;

        let total = response.result.len();
        let results: Vec<SearchResult> = response
            .result
            .into_iter()
            .filter_map(Self::map_scored_point)
            .collect();

        Self::log_nan_filtered(total, results.len());
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
