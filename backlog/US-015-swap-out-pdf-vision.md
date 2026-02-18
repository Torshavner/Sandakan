# Epic: Core Ingestion Reliability & Scalability

## Metadata

**Component**: CompositeFileLoader & ExtractorFactory (Layer 3: Infrastructure)
**Story Points**: 13

## Objective

Deploy interchangeable PDF extraction pipeline.
Guarantee crash-free ingestion yielding structured Markdown.
Enable cost-compute trade-offs via environment configuration (Local VLM vs Azure Document Intelligence).

---

## Architecture Pivot

**Issue**: Legacy parser (`pdf_oxide`) crashes on malformed headers.
**Solution**: Replace raw text stream parsing with visual/layout-aware extraction.
**Design**: Implement interchangeable adapters for `FileLoader` Port (Layer 2).
**Pathways**:

* Local Pipeline: GPU/CPU compute -> `pdfium-render` -> `candle` VLM.
* Cloud Pipeline: Managed service -> Azure AI Document Intelligence.

---

## Acceptance Criteria: Abstractions

* **Deprecate**: Remove `pdf_oxide` from codebase and dependency tree.
* **Standardize**: Define Layer 2 `FileLoader` trait accepting file bytes, returning Markdown strings.
* **Integrate**: Route Markdown output directly to `TextSplitter` -> `Embedder` without downstream component modifications.

---

## Acceptance Criteria: Local VLM Adapter

* **Rasterize**: Convert PDF bytes to image buffers via `pdfium-render`.
* **Process**: Transmit buffers to quantized local vision model using `candle` framework.
* **Output**: Return concatenated Markdown text payload.

---

## Acceptance Criteria: Cloud Adapter & Factory

* **Transmit**: Submit PDF bytes to Azure Document Intelligence `prebuilt-layout` endpoint via `reqwest`.
* **Poll**: Await Azure `Operation-Location` URL resolution.
* **Retrieve**: Extract native Markdown output.
* **Inject**: `ExtractorFactory` maps `APP_EXTRACTOR_PROVIDER` (`local_vlm` | `azure`) to `CompositeFileLoader` dependency.

---

## Implementation: Layer 2 Port Definition

```rust
#[async_trait]
pub trait FileLoader: Send + Sync {
    async fn extract_text(&self, file_bytes: &[u8], file_name: &str) -> Result<String, IngestionError>;
}

```

## Implementation: Layer 3 Azure Adapter

* Implement `AzureDocIntelAdapter`.
* Execute async polling loop using `tokio::time::sleep` handling HTTP 202 Accepted patterns.
* Target `api-version=2024-02-29-preview` utilizing `outputContentFormat=markdown` parameter.

---

## Implementation: Layer 3 Local Adapter & State

### Local VLM

* Implement `PdfiumRasterizer`: Map `&[u8]` -> `Vec<image::DynamicImage>`.
* Implement `CandleVlmAdapter`: Load `.gguf` weights, process frames iteratively, concatenate Markdown strings.

### State Configuration

* Update `Settings`: Append `ExtractorProvider` enum, `AZURE_DOC_INTEL_ENDPOINT`, `AZURE_DOC_INTEL_KEY`.
* Implement `ExtractorFactory::build(config: &Settings) -> Arc<dyn FileLoader>`.
* Inject selected `FileLoader` implementation into `AppState`.

---

## Non-Functional Requirements

* **Timeouts**: Configure `IngestionWorker` channels and `reqwest` HTTP clients to 300+ seconds.
* **Rate Limiting**: Execute exponential backoff handling Azure HTTP 429 responses.
* **Resource Limits**: Catch Local VLM Out-of-Memory (OOM) faults. Transition `JobStatus` -> `Failed` without thread panics.
* **Security**: Enforce HTTPS transmission. Load Azure API keys via secure environment variables.

---

## Testing Strategy

### Unit Testing

* Mock `reqwest` responses mapping Azure polling state transitions (running -> succeeded).
* Mock `PdfiumRasterizer` validating `CandleVlmAdapter` text concatenation algorithms.

### Integration Testing

* Execute Azure configuration with malformed PDF. Assert completion and layout parsing (e.g., tables).
* Execute Local VLM configuration with malformed PDF. Assert ingestion completion.

---
