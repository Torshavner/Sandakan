# Sandakan

An AI Agentic RAG system built in Rust as a capstone project for the Ciklum AI Academy Engineering Track.

Every major component — LLM, embedder, vector store, transcription, storage, eval — is defined as a trait and resolved at startup from config. The system can run entirely on local hardware or delegate to Azure services without changing application code.

---

## What it does

Sandakan ingests documents (PDF, text, audio, video), stores them as searchable vector chunks, and exposes a retrieval-augmented chat interface. On top of the RAG pipeline sits an agentic layer: a ReAct loop that can call tools (knowledge base search, file system, web, MCP servers), reason across multiple steps, and score its own output with a critic evaluator.

Core capabilities:

- Document ingestion with async background processing and job tracking
- Dense + sparse hybrid retrieval (BM25 + RRF) via Qdrant named vectors
- ReAct agent loop with configurable tool set and iteration budget
- Passive evaluation: every RAG query can emit an eval event scored offline by an LLM judge
- OpenAI-compatible chat completions endpoint (works with Open WebUI out of the box)
- Structured tracing with OTLP export; logs aggregated via Vector into Loki; dashboards in Grafana

---

## Architecture

Hexagonal (Ports & Adapters), four layers, dependencies point inward:

```
L4 Presentation  →  L3 Infrastructure  →  L2 Application  →  L1 Domain
(Axum handlers)      (Qdrant, Postgres,     (Services,          (Chunk, Document,
                      OpenAI, Candle)        Ports/Traits)       EvalEvent, ToolCall)
```

The domain and application layers have no knowledge of HTTP, databases, or external APIs. Infrastructure adapters implement application-layer traits and are wired together in `main.rs` at startup.

Detailed spec: [.ai/architecture.md](.ai/architecture.md)

Architecture diagram (D2 format): [src/architecture/diagram/system-architecture.d2](src/architecture/diagram/system-architecture.d2)

### Generating the SVG diagram

Install the D2 CLI: https://d2lang.com/tour/install

```bash
d2 src/architecture/diagram/system-architecture.d2 src/architecture/diagram/system-architecture.svg
```

With layout engine and theme:

```bash
d2 --theme 200 --layout elk src/architecture/diagram/system-architecture.d2 system-architecture.svg
```

---

## Stack

| Layer | Technology |
|---|---|
| Runtime | tokio 1.x |
| HTTP | axum 0.8, tower-http |
| Vector DB | Qdrant (gRPC, dense + sparse named vectors) |
| Relational DB | PostgreSQL 16 + sqlx (compile-time checked queries) |
| Local inference | candle (embeddings + Whisper transcription, Metal + Accelerate on Apple Silicon) |
| Remote LLM | OpenAI API / Azure OpenAI / LM Studio (config-driven) |
| PDF extraction | pdfium-render (local) or Azure Doc Intelligence or LM Studio VLM |
| Audio/Video | ffmpeg-sidecar + symphonia decoder |
| Sparse search | Custom BM25 (TF tokenizer + FNV-1a hash) → Qdrant sparse vectors |
| Hybrid fusion | Qdrant `PrefetchQueryBuilder` + `Fusion::Rrf` |
| Agent protocol | MCP (stdio + SSE) via `CompositeMcpClient` |
| Eval | LLM-as-judge faithfulness via outbox pattern (PostgreSQL) |
| Observability | tracing + OpenTelemetry OTLP → Tempo, Vector → Loki, Grafana |
| Config | config-rs, `appsettings.{env}.json` + `APP_*` env vars |

---

## Infrastructure (Docker)

All backing services are defined in [infrastructure/docker-compose.yml](infrastructure/docker-compose.yml).

```bash
cd infrastructure
docker compose up -d
```

Services:

| Service | Port | Purpose |
|---|---|---|
| postgres | 5432 | Job queue, conversations, eval events/outbox/results |
| pgadmin | 5050 | Postgres admin UI |
| qdrant | 6333 (REST), 6334 (gRPC) | Vector store |
| open-webui | 3000 | Chat UI (OpenAI-compatible, points at Sandakan on host:8080) |
| tempo | 3200, 4317 (OTLP gRPC), 4318 (OTLP HTTP) | Distributed tracing backend |
| loki | 3100 | Log aggregation |
| vector | 9000, 8686 | Log shipping (Docker → Loki) |
| grafana | 3001 | Dashboards (Loki + Tempo datasources pre-provisioned) |

All ports and credentials have defaults and can be overridden via `.env`:

```
POSTGRES_USER, POSTGRES_PASSWORD, POSTGRES_DB, POSTGRES_PORT
PGADMIN_EMAIL, PGADMIN_PASSWORD, PGADMIN_PORT
QDRANT_REST_PORT, QDRANT_GRPC_PORT
WEBUI_PORT
OPENAI_API_KEY, OPENAI_API_BASE_URL
```

---

## Running the Application

### Prerequisites

- Rust 1.85+
- Docker (for backing services)

**Apple Silicon (M1/M2/M3)** — Candle is compiled with `metal` and `accelerate` features (see `Cargo.toml`). Local embedding inference and Whisper transcription run on the Metal GPU via the Accelerate framework. The binary is native `aarch64-apple-darwin`; Rosetta is not required. The only prerequisite is Xcode Command Line Tools:

```bash
xcode-select --install
```

If you see linker errors referencing `Accelerate.framework` or `Metal`, the full CLI tools are likely missing (the App Store stub is not sufficient).

**ffmpeg** — required for audio and video ingestion. `ffmpeg-sidecar` expects the `ffmpeg` binary on `PATH` at runtime.

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
apt-get install ffmpeg
```

**libpdfium** — required only when `extraction.pdf.provider = "pdfium"` (local PDF rasterization). `pdfium-render` loads the shared library at runtime, not at compile time.

```bash
# macOS
# Download from https://github.com/bblanchon/pdfium-binaries/releases
cp libpdfium.dylib /usr/local/lib/
# or point to it at runtime:
export DYLD_LIBRARY_PATH=/path/to/pdfium-dir:$DYLD_LIBRARY_PATH

# Linux
cp libpdfium.so /usr/local/lib/ && ldconfig
```

To avoid managing this dependency locally, switch PDF extraction to Azure Document Intelligence — see the Azure setup section below.

### Build and run

```bash
# Start backing services
cd infrastructure && docker compose up -d && cd ..

# Set environment
export APP_ENVIRONMENT=local

# Run (DB migrations applied automatically on startup)
cargo run --release
```

Server starts on `http://127.0.0.1:8080` by default.

### Configuration

Config is loaded from `appsettings.{APP_ENVIRONMENT}.json`, then overridden by `APP_*` env vars (separator: `_`).

Key flags:

| Setting | Default | Effect |
|---|---|---|
| `qdrant.hybrid_search` | false | Enables BM25 + dense + RRF. Requires re-indexing. |
| `agent.enabled` | true | Enables the ReAct agent endpoint |
| `agent.semantic_tools` | false | Dynamic tool selection via embedding similarity |
| `agent.dynamic_tools_description` | true | Tool descriptions derived from registry at runtime |
| `eval.enabled` | false | Passive faithfulness scoring via background worker |
| `extraction.pdf.provider` | lm_studio | `lm_studio`, `azure_doc_intel`, or `pdfium` |
| `extraction.audio.enabled` | true | Transcription on ingest |

When enabling `qdrant.hybrid_search`, the Qdrant collection must be recreated with named vectors (`"dense"` + `"sparse"`). Set `rag.similarity_threshold` near `0` — RRF scores are not cosine similarities (typical range ~0.01–0.05).

### Azure setup (cloud, no native dependencies)

With Azure, `libpdfium` and local Candle Whisper inference are not required. Set the following env vars (or equivalent keys in `appsettings.Prod.json`):

```bash
# LLM
APP_LLM__PROVIDER=azure
APP_LLM__AZURE_ENDPOINT=https://<your-resource>.openai.azure.com/
APP_LLM__API_KEY=<key>
APP_LLM__CHAT_MODEL=<deployment-name>            # e.g. gpt-4o

# Embeddings
APP_EMBEDDINGS__PROVIDER=openai
APP_EMBEDDINGS__API_KEY=<key>
APP_EMBEDDINGS__BASE_URL=https://<your-resource>.openai.azure.com/
APP_EMBEDDINGS__MODEL=text-embedding-3-small

# PDF extraction via Azure Document Intelligence (no libpdfium needed)
APP_EXTRACTION__PDF__PROVIDER=azure_doc_intel
APP_EXTRACTION__PDF__AZURE_ENDPOINT=https://<your-resource>.cognitiveservices.azure.com/
APP_EXTRACTION__PDF__AZURE_KEY=<key>

# Audio transcription via Azure Whisper (no Candle inference needed)
APP_EXTRACTION__AUDIO__PROVIDER=azure
APP_EXTRACTION__AUDIO__AZURE_ENDPOINT=https://<your-resource>.openai.azure.com/
APP_EXTRACTION__AUDIO__AZURE_KEY=<key>
APP_EXTRACTION__AUDIO__AZURE_MODEL=whisper
```

`ffmpeg` is still required on the host for MP4 ingestion — audio extraction from video happens locally before the transcription call.

---

## API

Full request examples in [collections/](collections/) (Hurl format).

| Endpoint | Method | Description |
|---|---|---|
| `/health` | GET | Liveness check |
| `/api/v1/ingest` | POST | Multipart file upload (PDF, text, MP3, WAV, MP4) |
| `/api/v1/ingest-reference` | POST | Ingest content from a URL |
| `/api/v1/query` | POST | RAG query, returns context chunks + answer |
| `/api/v1/jobs/{id}` | GET | Poll ingestion job status |
| `/api/v1/agent/chat` | POST | Agentic chat with tool calling (SSE) |
| `/v1/chat/completions` | POST | OpenAI-compatible chat completions (streaming) |
| `/v1/models` | GET | Model listing |

Run all E2E collections:

```bash
hurl --variable base_url=http://127.0.0.1:8080 \
     --file-root collections/ \
     collections/*.hurl
```

---

## Validation

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

---

## Swappable Components

To replace any provider, implement the corresponding L2 trait in `src/application/ports/` and register the adapter in `main.rs`. No application or domain code changes required.

| Trait | Current implementations |
|---|---|
| `Embedder` | `OpenAiEmbedder`, `LocalCandleEmbedder` |
| `SparseEmbedder` | `Bm25SparseEmbedder` |
| `LlmClient` | `StreamingLlmClient` (OpenAI / Azure OpenAI / LM Studio) |
| `VectorStore` | `QdrantAdapter` |
| `TranscriptionEngine` | `OpenAiWhisperEngine`, `AzureWhisperEngine`, `CandleWhisperEngine` |
| `TextSplitter` | `SemanticSplitter`, `RecursiveCharacterSplitter`, `MarkdownSemanticSplitter` |
| `FileLoader` | `CompositeFileLoader` (PDF via pdfium / Azure Doc Intelligence / VLM, text, audio, video) |
| `ToolRegistry` | `StaticToolRegistry`, `SemanticToolRegistry` |
| `McpClientPort` | `StdioMcpClient`, `SseMcpClient`, `CompositeMcpClient` |
| `EvalEventRepository` | `PgEvalEventRepository`, `JsonlEvalEventRepository` |
