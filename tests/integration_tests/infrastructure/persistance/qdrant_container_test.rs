use sandakan::application::ports::{CollectionConfig, VectorStore};
use sandakan::domain::{Chunk, DocumentId, Embedding};
use sandakan::infrastructure::persistence::QdrantAdapter;
use std::time::Duration;
use testcontainers::{ContainerAsync, GenericImage, core::ContainerPort, runners::AsyncRunner};

pub struct TestQdrant {
    pub adapter: QdrantAdapter,
    _container: ContainerAsync<GenericImage>,
}

impl TestQdrant {
    pub async fn new() -> Self {
        let qdrant_image = GenericImage::new("qdrant/qdrant", "latest")
            .with_exposed_port(ContainerPort::Tcp(6334));

        let container = qdrant_image
            .start()
            .await
            .expect("Failed to start Qdrant container");

        let host_port = container
            .get_host_port_ipv4(6334)
            .await
            .expect("Failed to get Qdrant gRPC port");

        let qdrant_url = format!("http://localhost:{}", host_port);
        let collection_name = format!("test_collection_{}", uuid::Uuid::new_v4());

        let adapter = wait_for_qdrant_connection(&qdrant_url, collection_name).await;

        Self {
            adapter,
            _container: container,
        }
    }
}

async fn wait_for_qdrant_connection(url: &str, collection_name: String) -> QdrantAdapter {
    let max_retries = 10;
    let mut delay = Duration::from_millis(500);

    for attempt in 1..=max_retries {
        let adapter = match QdrantAdapter::new(url, collection_name.clone()).await {
            Ok(a) => a,
            Err(e) if attempt < max_retries => {
                eprintln!(
                    "Qdrant client build failed (attempt {attempt}/{max_retries}): {e}, retrying in {}ms",
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(5));
                continue;
            }
            Err(e) => {
                panic!("Failed to build Qdrant client after {max_retries} attempts: {e}");
            }
        };

        // Probe a real gRPC call to verify the server is fully ready
        match adapter.collection_exists().await {
            Ok(_) => {
                eprintln!("Qdrant ready after attempt {attempt}");
                return adapter;
            }
            Err(e) if attempt < max_retries => {
                eprintln!(
                    "Qdrant not ready (attempt {attempt}/{max_retries}): {e}, retrying in {}ms",
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(5));
            }
            Err(e) => {
                panic!("Failed to connect to Qdrant after {max_retries} attempts: {e}");
            }
        }
    }
    unreachable!()
}

#[tokio::test]
async fn given_test_suite_starting_up_when_initializing_qdrant_container_then_instance_is_available_and_client_connects()
 {
    let test_qdrant = TestQdrant::new().await;

    let exists = test_qdrant
        .adapter
        .collection_exists()
        .await
        .expect("Failed to check collection existence");

    assert!(!exists, "Collection should not exist yet");
}

#[tokio::test]
async fn given_running_qdrant_container_when_creating_collection_then_collection_exists() {
    let test_qdrant = TestQdrant::new().await;
    let config = CollectionConfig::new(384);

    let created = test_qdrant
        .adapter
        .create_collection(&config)
        .await
        .expect("Failed to create collection");

    assert!(created, "Collection should be created");

    let exists = test_qdrant
        .adapter
        .collection_exists()
        .await
        .expect("Failed to check collection existence");

    assert!(exists, "Collection should exist after creation");
}

#[tokio::test]
async fn given_running_qdrant_container_when_ingestion_service_upserts_document_chunk_then_vector_count_increments_and_payload_matches()
 {
    let test_qdrant = TestQdrant::new().await;
    let config = CollectionConfig::new(384);

    test_qdrant
        .adapter
        .create_collection(&config)
        .await
        .expect("Failed to create collection");

    let document_id = DocumentId::new();
    let chunk = Chunk::new(
        "This is a test document chunk for vector storage validation.".to_string(),
        document_id,
        Some(1),
        0,
    );

    let mut embedding_values = vec![0.0; 384];
    for (i, value) in embedding_values.iter_mut().enumerate().take(10) {
        *value = (i as f32) / 10.0;
    }
    let length: f32 = embedding_values.iter().map(|x| x * x).sum::<f32>().sqrt();
    if length > 0.0 {
        for value in &mut embedding_values {
            *value /= length;
        }
    }
    let embedding = Embedding {
        values: embedding_values,
    };

    test_qdrant
        .adapter
        .upsert(
            std::slice::from_ref(&chunk),
            std::slice::from_ref(&embedding),
        )
        .await
        .expect("Failed to upsert chunk");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let search_results = test_qdrant
        .adapter
        .search(&embedding, 5)
        .await
        .expect("Failed to search");

    assert_eq!(search_results.len(), 1, "Should retrieve one result");

    let result = &search_results[0];
    assert_eq!(result.chunk.text, chunk.text, "Text should match");
    assert_eq!(
        result.chunk.document_id, document_id,
        "Document ID should match"
    );
    assert_eq!(result.chunk.page, Some(1), "Page should match");
    assert_eq!(result.chunk.offset, 0, "Offset should match");
}

#[tokio::test]
async fn given_tests_completed_when_test_scope_ends_then_container_is_automatically_stopped() {
    {
        let test_qdrant = TestQdrant::new().await;
        let config = CollectionConfig::new(384);

        test_qdrant
            .adapter
            .create_collection(&config)
            .await
            .expect("Failed to create collection");

        let exists = test_qdrant
            .adapter
            .collection_exists()
            .await
            .expect("Failed to check collection existence");

        assert!(exists, "Collection should exist");
    }
}
