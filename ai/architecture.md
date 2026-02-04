## System Architecture: Rust RAG Pipeline

### 1. Design Philosophy

* **Architecture**: Clean / Hexagonal. Decouples core logic from external adapters (Qdrant, OpenAI).
* **Type Safety**: Static typing prevents data ingestion/serialization errors.
* **Concurrency**: `tokio` for async I/O; `rayon` for CPU-bound tasks (Whisper/Parsing).
* **Modularity**: Interface-based design allows hot-swapping Vector DBs or LLMs.

### 2. Dependency Mapping

* **Domain (L1)**: Internal structs (Chunk, Document). No external dependencies.
* **Application (L2)**: Business logic and Ports (Traits). Defines system capabilities.
* **Infrastructure (L3)**: Concrete Adapters. Implements Ports for Qdrant, OpenAI, Whisper, PDF.
* **Presentation (L4)**: Composition root (`main.rs`), CLI, and REST API (Axum).
* **Direction**: L4 -> L3 -> L2 -> L1 (Dependencies point inward only).

### 3. Component Specifications

#### Layer 1 & 2: Core Logic

* **domain/entities.rs**: Defines `Chunk` (text, metadata, page).
* **application/ports/**: Abstract traits for `FileLoader`, `VectorStore`, `LlmClient`.
* **application/services/**: `IngestionService` (File -> Vectors) and `RetrievalService` (Query -> Answer).

#### Layer 3 & 4: Implementation

* **infrastructure/persistence**: Qdrant gRPC adapter; maps `Domain::Chunk` to `PointStruct`.
* **infrastructure/llm**: OpenAI client for embeddings and completions.
* **infrastructure/fs**: PDF text extraction and Whisper audio transcription.
* **main.rs**: Orchestrates DI, initializes `tokio`, and binds routes.

### 4. Data Workflows

#### Ingestion (Write Path)

* **Trigger**: API Upload (PDF/Audio).
* **Extraction**: Async parallel text extraction via PDF/Whisper adapters.
* **Processing**: Recursive text splitting into semantic chunks.
* **Vectorization**: Batch embedding generation via LLM client.
* **Storage**: Atomic upsert to Qdrant vector store.

#### Retrieval (Read Path)

* **Query**: Natural language input -> LLM Embedding.
* **Search**: Vector similarity search (Top-K) in Qdrant.
* **Augmentation**: Context construction from retrieved chunks + system prompt.
* **Generation**: LLM chat completion -> Final response.

### 5. Technical Stack

| Component | Choice | Rationale |
| --- | --- | --- |
| Runtime | `tokio` | Standard for async I/O and concurrent API calls. |
| Vector DB | Qdrant | Rust-native, high-performance, gRPC support. |
| Serialization | `serde` | Zero-copy mapping between Domain and DB payloads. |
| Observability | `tracing` | Context-aware logging for async call stacks. |
| Inference | `candle` | Local ML (Whisper/Embeddings) without Python overhead. |

### 6. Optimization & Scaling

* **Hybrid Search**: Future integration of BM25 (sparse) + Dense vectors in Qdrant.
* **Streaming**: Refactor `ChatService` to `Stream<Item = String>` for real-time tokens.
* **Lifecycle**: Implementation of TTL or GC for stale document chunks.

Would you like me to generate the Rust trait definitions for the Application Ports?