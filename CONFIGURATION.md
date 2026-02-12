# Configuration System

## Overview

The application uses a hierarchical configuration system based on environment-specific JSON files and environment variable overrides.

## Environment Types

Three environments are supported:
- **Local**: Development environment with debug logging and UDP telemetry
- **Test**: Testing environment with structured JSON logging
- **Prod**: Production environment with optimized settings and larger models

## Configuration Files

- `appsettings.Local.json`: Local development settings
- `appsettings.Test.json`: Testing environment settings
- `appsettings.Prod.json`: Production environment settings

## Configuration Loading

Settings are loaded in this order (later sources override earlier ones):

1. Environment-specific JSON file (e.g., `appsettings.Local.json`)
2. Environment variables with `APP_` prefix

### Environment Variable Format

Use `APP_` prefix with `_` separator for nested values:

```bash
APP_SERVER_PORT=3000
APP_QDRANT_URL=http://localhost:6334
APP_LLM_API_KEY=sk-...
APP_EMBEDDINGS_MODEL=text-embedding-3-small
APP_EMBEDDINGS_STRATEGY=semantic
APP_CHUNKING_MAX_CHUNK_SIZE=512
```

## Configuration Structure

### Server Settings
- `host`: Bind address (default: `127.0.0.1` for Local, `0.0.0.0` for Test/Prod)
- `port`: HTTP port (default: `3000` for Local, `8080` for Test/Prod)

### Qdrant Settings
- `url`: Qdrant connection URL
- `collection_name`: Vector collection name

### Embeddings Settings
- `model`: OpenAI embedding model name
- `strategy`: Chunking strategy (`semantic` or `fixed`)
- `dimension`: Embedding vector dimension (1536 for small, 3072 for large)
- `chunk_overlap`: Number of overlapping tokens between chunks

### Chunking Settings
- `max_chunk_size`: Maximum tokens per chunk
- `overlap_tokens`: Overlap between consecutive chunks

### LLM Settings
- `api_key`: OpenAI API key (required, set via environment variable)
- `chat_model`: Chat completion model
- `max_tokens`: Maximum tokens in response
- `temperature`: Sampling temperature (0.0-1.0)

### Logging Settings
- `level`: Log level (`debug`, `info`, `warn`, `error`)
- `enable_json`: Structured JSON logging
- `enable_udp`: UDP telemetry output

### Extraction Settings

#### PDF Extraction
- `enabled`: Enable PDF processing
- `max_file_size_mb`: Maximum PDF file size

#### Audio Extraction
- `enabled`: Enable audio transcription
- `max_file_size_mb`: Maximum audio file size
- `whisper_model`: Whisper model variant (`base`, `medium`, etc.)

## Usage

### Set Environment

```bash
export APP_ENVIRONMENT=local  # or test, prod
```

### Run Application

```bash
cargo run
```

The application will:
1. Load `.env` file if present (via `dotenvy`)
2. Read `APP_ENVIRONMENT` variable (defaults to `local`)
3. Load corresponding `appsettings.{Environment}.json`
4. Apply environment variable overrides

## Security

**Never commit `.env` files containing secrets.**

Use `.env.example` as a template and set sensitive values via environment variables:

```bash
cp .env.example .env
# Edit .env and set APP_LLM_API_KEY
```
