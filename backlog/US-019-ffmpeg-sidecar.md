### **User Story: Replace Symphonia with FFmpeg-Sidecar for RAG Media Ingestion**

**Title:** Refactor media ingestion layer: Replace Symphonia with FFmpeg CLI wrapper

**Description:**
**As a** backend developer building the RAG ingestion pipeline,
**I want to** replace the pure-Rust `symphonia` demuxer with the `ffmpeg-sidecar` crate,
**So that** the pipeline can reliably extract both H.264 video keyframes and resampled AAC audio without encountering unsupported codec errors or risking main-thread crashes on malformed files.

---

### **Acceptance Criteria**

* **Dependency Cleanup:** Remove `symphonia` and its associated audio-only decoding dependencies from `Cargo.toml`.
* **Audio Extraction:** The ingestion module uses `ffmpeg-sidecar` to extract audio from the source media, outputting it as a 16kHz, mono WAV stream optimized for the downstream Speech-to-Text model.
* **Video Sampling:** The ingestion module uses `ffmpeg-sidecar` to extract raw RGB frames at a predefined, low frame rate (e.g., `fps=1`) and scales them down (e.g., `scale=720:-1`) to generate visual embeddings.
* **Fault Tolerance:** The Rust application must catch and log sub-process failures if a corrupted video file causes FFmpeg to crash, allowing the main ingestion queue to continue processing the next file safely.
* **Environment Validation:** Implement a startup check to verify that the `ffmpeg` CLI binary is installed and accessible in the system `$PATH`, throwing a clear startup error if it is missing.

---

### **Architectural Fit & Context**

Switching to `ffmpeg-sidecar` strongly aligns with standard RAG ETL (Extract, Transform, Load) principles.

Because we are doing offline batch ingestion rather than real-time media playback, process isolation is more valuable than zero-latency execution. By wrapping the external FFmpeg binary, we isolate the computationally heavy and risky C-level decoding process from our main Rust application. If a malformed MP4 triggers a segmentation fault during decoding, the FFmpeg sub-process dies, but our Rust application catches the exit code and remains perfectly stable.
