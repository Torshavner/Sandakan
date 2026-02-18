# Audio Pre-processing: Voice Activity Detection (VAD)

## Requirement Definition
As a system administrator, I need the ingestion pipeline to automatically detect and remove silent segments from uploaded audio and video files before they are transcribed so that I can drastically reduce transcription API compute costs and prevent Whisper from hallucinating fake text during dead air.

## Problem Statement
* **Current bottleneck/technical debt:** The current `IngestionService` sends raw, unedited audio buffers to the `TranscriptionEngine`. Whisper models frequently "hallucinate" repetitive text (e.g., "Thank you for watching") when processing long periods of ambient noise or silence.
* **Performance/cost implications:** Processing silent segments increases inference time by 20-30% on average and inflates token usage in downstream LLM contexts due to "garbage" text.
* **Architectural necessity:** To maintain a "Clean Architecture," audio filtering must be decoupled from the transcription logic, ensuring the `TranscriptionEngine` only receives high-signal data.

## Acceptance Criteria (Gherkin Enforced)
### Voice Activity Identification
* **Given** a user uploads a media file via `POST /api/v1/ingest`,
* **When** the `IngestionWorker` decodes the media into a 16kHz PCM buffer,
* **Then** the system must run a VAD model (Silero) to identify all segments of active human speech.

### Silence Truncation logic
* **Given** a mapped list of active speech segments,
* **When** a period of silence exceeds the configured threshold (default: 1.5 seconds),
* **Then** that silent period must be excluded from the final audio buffer sent to the `TranscriptionEngine`.

### Hallucination Mitigation
* **Given** the processed audio is sent to the `TranscriptionEngine` (via `candle`),
* **When** the transcription is returned,
* **Then** it must contain zero repetitive hallucinated phrases caused by background room noise or dead air.

* **Technical Metric:** Audio buffer size sent to inference must be reduced by ≥ 95% of the total identified silent duration.
* **Observability:** Emit a `tracing` event `vad_silence_removed_seconds` for every ingestion task.

## Technical Context
* **Architectural patterns:** Pipeline Pattern (Filter/Adapter), Strategy Pattern for `VadProvider`.
* **Stack components:** Rust, `tokio` (Async I/O), `candle-onnx` (Silero VAD), `ffmpeg-next` or `symphonia` (Decoding).
* **Integration points:** `infrastructure/fs` (Audio Decoder) and `infrastructure/llm` (Whisper/Candle).
* **Namespace/Config:** `vad.silence_threshold_ms: 1500`, `vad.sample_rate: 16000`.

## Cross-Language Mapping
* `VadFilter` ≈ Audio Gate / Squelch (Digital Signal Processing)

## Metadata
* **Dependencies:** `infrastructure/fs` (Audio Transcription)
* **Complexity:** High
* **Reasoning:** Requires managing ONNX runtime sessions within a multi-threaded `tokio` environment and handling memory-efficient PCM buffer manipulation without excessive cloning.

## Quality Benchmarks
## Test-First Development Plan
- [ ] Parse criteria into Given-When-Then scenarios.
- [ ] Generate failing test suite: `tests/infrastructure/fs/vad_filter_tests.rs`.
- [ ] Execute `cargo test` to confirm failure (missing `VadFilter` implementation).
- [ ] Implement minimal `silero-vad` logic in `infrastructure/fs/vad_filter.rs`.
- [ ] Refactor under green state to ensure zero-copy buffer passing.