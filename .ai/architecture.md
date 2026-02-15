## System Architecture: Rust RAG Pipeline

### 1. Design Philosophy

* **Architecture**: Clean / Hexagonal. Decouples core logic from external adapters (Qdrant, PostgreSQL, OpenAI).
* **Type Safety**: Newtype IDs (`ChunkId`, `DocumentId`, `ConversationId`, `JobId`, `MessageId`) and domain enums prevent misuse.
* **Concurrency**: `tokio` for async I/O; background `IngestionWorker` via `mpsc` channel for decoupled processing.
* **Modularity**: Trait-based ports with factory patterns allow hot-swapping providers (LLM, embeddings, transcription, chunking).
* **Streaming**: SSE token streaming with `tokio::select!` multiplexing and keep-alive.

### 2. Dependency Mapping

* **Domain (L1)**: Pure value objects and entities (`Chunk`, `Document`, `Job`, `Conversation`, `Message`, `Embedding`). No external dependencies.
* **Application (L2)**: Business logic, Ports (Traits), and Services. Defines system capabilities and error types.
* **Infrastructure (L3)**: Concrete Adapters. Implements Ports for Qdrant, PostgreSQL, OpenAI/Azure, Candle, PDF, Audio.
* **Presentation (L4)**: Composition root (`main.rs`), Axum REST API, handlers, config, and state management.
* **Direction**: L4 -> L3 -> L2 -> L1 (Dependencies point inward only).

### 3. Component Specifications

#### Layer 1: Domain

* **domain/chunk.rs**: `Chunk`, `ChunkId`, `DocumentId` — text segment with metadata (page, offset).
* **domain/document.rs**: `Document`, `ContentType` enum (Pdf, Audio, Video, Text).
* **domain/job.rs**: `Job` — async ingestion job tracking.
* **domain/job_status.rs**: `JobStatus` enum (Queued, Processing, MediaExtraction, Transcribing, Embedding, Completed, Failed).
* **domain/conversation.rs**: `Conversation` — RAG conversation context.
* **domain/message.rs**: `Message` — chat message with role and content.
* **domain/message_role.rs**: `MessageRole` enum (System, User, Assistant).
* **domain/embedding.rs**: `Embedding` — vector with cosine similarity calculation.
* **domain/conversation_id.rs**, **job_id.rs**, **message_id.rs**: Strong-typed UUID wrappers.

#### Layer 2: Application

**Ports (Traits):**

* `FileLoader`: Extract text from uploaded files (PDF, plain text).
* `VectorStore`: Collection CRUD, upsert embeddings, semantic search, delete.
* `Embedder`: Single and batch text-to-embedding generation.
* `LlmClient`: Chat completions with `complete()` and `complete_stream()`.
* `TextSplitter`: Split text into chunks (semantic or fixed-size strategies).
* `TranscriptionEngine`: Audio-to-text transcription.
* `JobRepository`: Job lifecycle persistence (create, status updates, queries).
* `ConversationRepository`: Conversation and message history persistence.

**Services:**

* `IngestionService`: Synchronous document ingestion (extract -> split -> embed -> store).
* `IngestionWorker`: Async background worker receiving `IngestionMessage` via channel; handles audio/video transcription pipeline with job status progression.
* `RetrievalService`: RAG query execution (embed -> search -> filter by threshold -> token budget -> LLM completion); supports both batch and streaming responses.
* `token_counter`: Token counting via `tiktoken-rs` for context window management.

#### Layer 3: Infrastructure

**Persistence:**

* `QdrantAdapter` implements `VectorStore` — gRPC client; maps `Chunk` + `Embedding` to `PointStruct`; payload indexes on `document_id`, `file_type`, `tenant_id`.
* `PgJobRepository` implements `JobRepository` — PostgreSQL via `sqlx`.
* `PgConversationRepository` implements `ConversationRepository` — PostgreSQL via `sqlx`.

**LLM:**

* `StreamingLlmClient` implements `LlmClient` — multi-provider (OpenAI, Azure, LMStudio) streaming chat completions via `reqwest` with SSE parsing. Provider-specific auth (Bearer vs `api-key` header).
* `OpenAiEmbedder` implements `Embedder` — OpenAI `/v1/embeddings` with rate-limit handling.
* `LocalCandleEmbedder` implements `Embedder` — local CPU inference via Candle ML framework; downloads models from Hugging Face Hub.
* `EmbedderFactory` — creates embedder based on config (Local or OpenAI).

**Audio:**

* `CandleWhisperEngine` implements `TranscriptionEngine` — local Whisper inference via Candle; loads model, tokenizer, mel filters from Hugging Face Hub.
* `OpenAiWhisperEngine` implements `TranscriptionEngine` — remote OpenAI Whisper API.
* `TranscriptionEngineFactory` — creates engine based on config (Local or OpenAI).
* `audio_decoder` — decodes audio formats (MP3, OGG, WAV, FLAC, AAC) via Symphonia; resamples to 16kHz PCM via Rubato.

**Text Processing:**

* `CompositeFileLoader` implements `FileLoader` — routes extraction by `ContentType` (PDF -> `PdfAdapter`, Text -> `PlainTextAdapter`).
* `PdfAdapter` — extracts text per page via `pdf_oxide` with 30s timeout.
* `PlainTextAdapter` — passthrough with sanitization.
* `RecursiveCharacterSplitter` implements `TextSplitter` — fixed-size token-based chunking with overlap via `tiktoken-rs`.
* `SemanticSplitter` implements `TextSplitter` — similarity-based chunking at sentence boundaries using embedder.
* `TextSplitterFactory` — creates splitter based on `ChunkingStrategy` config (Semantic or Fixed).
* `text_sanitizer` — removes control chars, extra whitespace, normalizes unicode.

**Observability:**

* `init_tracing` — structured JSON logging via `tracing-subscriber` with env filter.
* `request_id_middleware` — injects `REQUEST_ID` header on all requests.
* `prompt_sanitizer` — truncates sensitive data before tracing.

#### Layer 4: Presentation

**Handlers:**

* `POST /api/v1/ingest` — multipart file upload; creates `Document` + `Job`; sends to async worker channel; returns immediately.
* `POST /v1/chat/completions` — OpenAI-compatible chat endpoint; SSE streaming with keep-alive; conversation history persistence.
* `POST /api/v1/query` — RAG query; returns answer + source chunks with scores.
* `GET /api/v1/jobs/{job_id}` — job status polling.
* `GET /health` — liveness probe.
* `GET /v1/models` — lists available models.

**Infrastructure:**

* `AppState<F, L, V, T>` — generic Axum state holding `Arc`-wrapped services, repos, config, and ingestion channel sender.
* `Settings` — strongly-typed config from `appsettings.{env}.json` + `APP_*` env vars.
* `ScaffoldConfig` — toggles echo mode for testing without LLM.
* Router with CORS, tracing, and request-id middleware layers.

### 4. Data Workflows

#### Ingestion (Write Path)

* **Trigger**: `POST /api/v1/ingest` multipart upload (PDF, Audio, Video, Text).
* **Async Handoff**: Creates `Job` (Queued) and sends `IngestionMessage` to background worker via `mpsc` channel.
* **Media Extraction** (Audio/Video): Decode audio -> resample to 16kHz PCM -> transcribe via Whisper engine.
* **Text Extraction** (PDF/Text): Extract via `CompositeFileLoader` routing by `ContentType`.
* **Chunking**: Split text via configured strategy (semantic similarity or fixed-size with overlap).
* **Embedding**: Batch embedding generation via configured embedder (local Candle or OpenAI API).
* **Storage**: Atomic upsert to Qdrant with payload indexes.
* **Job Status Progression**: Queued -> Processing -> MediaExtraction -> Transcribing -> Embedding -> Completed (or Failed).

#### Retrieval (Read Path)

* **Query**: Natural language input via chat completions or query endpoint.
* **Embedding**: Question -> vector via embedder.
* **Search**: Cosine similarity search (Top-K) in Qdrant, filtered by similarity threshold.
* **Token Budget**: Accumulate context chunks until `max_context_tokens` reached.
* **Augmentation**: Construct prompt from system template + context chunks + user question.
* **Generation**: Streaming or batch LLM completion via `StreamingLlmClient`.
* **Persistence**: Store user + assistant messages in conversation history (PostgreSQL).

### 5. Technical Stack

| Component | Choice | Rationale |
| --- | --- | --- |
| Runtime | `tokio` | Standard for async I/O, channels, and concurrent API calls. |
| HTTP | `axum` | Ergonomic Rust web framework with tower middleware. |
| Vector DB | Qdrant | Rust-native, high-performance, gRPC support. |
| Relational DB | PostgreSQL (`sqlx`) | Job tracking, conversation history, migrations. |
| Serialization | `serde` | Zero-copy mapping between domain and DB/API payloads. |
| Observability | `tracing` | Context-aware structured logging for async call stacks. |
| Local Inference | `candle` | CPU-based Whisper transcription and embeddings without Python. |
| Audio Decoding | `symphonia` + `rubato` | Multi-format audio decode and 16kHz resampling. |
| PDF Extraction | `pdf_oxide` | Pure Rust PDF text extraction. |
| Tokenization | `tiktoken-rs` | GPT tokenizer for context window management. |
| LLM Streaming | `reqwest` + SSE | Direct multi-provider streaming (OpenAI, Azure, LMStudio). |

### 6. Architectural Patterns

* **Dependency Injection**: Trait-based ports resolved in composition root (`main.rs`).
* **Factory Pattern**: `EmbedderFactory`, `TextSplitterFactory`, `TranscriptionEngineFactory` for config-driven provider selection.
* **Strategy Pattern**: `ChunkingStrategy` (Semantic vs Fixed) and `EmbeddingProvider` (Local vs OpenAI).
* **Composite Pattern**: `CompositeFileLoader` routes by `ContentType`.
* **Repository Pattern**: `JobRepository`, `ConversationRepository` abstract persistence.
* **Actor Pattern**: `IngestionWorker` as background task with `mpsc::channel(64)`.
* **Value Objects**: Strong-typed UUID wrappers prevent ID misuse across aggregates.

### 7. Optimization & Scaling

* **Hybrid Search**: Future integration of BM25 (sparse) + Dense vectors in Qdrant.
* **Lifecycle**: Implementation of TTL or GC for stale document chunks.
* **Graceful Shutdown**: Store worker `JoinHandle` for clean SIGTERM handling.
