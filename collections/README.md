# API Collections (Hurl)

E2E test collections for the Sandakan RAG API using [Hurl](https://hurl.dev).

## Prerequisites

```bash
brew install hurl
```

## Usage

```bash
# Run a single collection
hurl --variable base_url=http://127.0.0.1:3000 --file-root collections/ collections/health.hurl

# Run all collections
hurl --variable base_url=http://127.0.0.1:3000 --file-root collections/ collections/*.hurl

# Verbose output
hurl --very-verbose --variable base_url=http://127.0.0.1:3000 --file-root collections/ collections/e2e-ingest-and-query.hurl
```

> `--file-root collections/` is required so file references in multipart uploads resolve correctly.

## Collections

| File | Endpoints | Description |
|------|-----------|-------------|
| `health.hurl` | `GET /health` | Server liveness check |
| `models.hurl` | `GET /v1/models`, `/api/models` | Model listing |
| `ingest-pdf.hurl` | `POST /api/v1/ingest` | PDF upload (multipart) |
| `ingest-text.hurl` | `POST /api/v1/ingest` | Plain text upload (multipart) |
| `query.hurl` | `POST /api/v1/query` | RAG query with optional conversation_id |
| `chat-completions.hurl` | `POST /v1/chat/completions`, `/api/chat/completions` | OpenAI-compatible chat (non-streaming) |
| `chat-streaming.hurl` | `POST /v1/chat/completions` | SSE streaming chat |
| `e2e-ingest-and-query.hurl` | All of the above | Full flow: health → ingest → query → chat |
| `error-cases.hurl` | Various | Invalid requests and error responses |

## Test Fixtures

- `sample-rag-docs.pdf` — Small PDF with RAG pipeline documentation
- `sample-notes.txt` — Plain text lecture notes about RAG
