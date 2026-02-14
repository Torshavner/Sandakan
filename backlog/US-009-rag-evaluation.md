# Simple RAG Evaluation Pipeline

## Requirement Definition
As a **RAG System Developer**, I need **an automated evaluation harness for retrieval and generation quality** so that **I can measure regression, compare configurations, and validate that changes to chunking, embedding, or prompts actually improve answer quality.**

## Problem Statement
* **Current bottleneck/technical debt:** There is no quantitative way to assess whether the RAG pipeline produces correct, grounded answers. Quality is evaluated manually by eyeballing responses, which is unreliable and does not scale.
* **Performance/cost implications:** Without evaluation metrics, changes to chunking strategy, similarity thresholds, top-k, or prompt templates are deployed blind — a single bad config change could degrade answer quality for all users without detection.
* **Architectural necessity:** A lightweight evaluation loop is the prerequisite for any iterative improvement cycle (prompt tuning, embedding model selection, retrieval parameter optimization).

## Acceptance Criteria (Gherkin Enforced)

### 1. Evaluation Dataset
* **Given** a JSONL file containing evaluation entries with fields `question`, `expected_answer`, and `expected_source_pages` (optional),
* **When** the evaluation harness is invoked,
* **Then** it must parse and validate all entries, rejecting malformed rows with a clear error message.

### 2. Retrieval Quality (Context Recall)
* **Given** an evaluation entry with `expected_source_pages: [3, 5]`,
* **When** the system queries the RAG pipeline and receives source chunks,
* **Then** it must compute **context recall** as the fraction of expected pages found in the returned sources,
* **And** log per-question recall and aggregate mean recall across the dataset.

### 3. Answer Faithfulness (Grounding Check)
* **Given** a generated answer and the retrieved context chunks,
* **When** the evaluator checks faithfulness,
* **Then** it must use an LLM-as-judge prompt to score whether the answer is fully supported by the context (score: 0.0–1.0),
* **And** flag any answer scoring below a configurable `faithfulness_threshold` (default: 0.7).

### 4. Answer Correctness (Semantic Similarity)
* **Given** a generated answer and the `expected_answer` from the evaluation dataset,
* **When** the evaluator checks correctness,
* **Then** it must compute **cosine similarity** between the embeddings of both answers,
* **And** report per-question similarity and aggregate mean across the dataset.

### 5. Evaluation Report
* **Given** a completed evaluation run,
* **When** the harness finishes,
* **Then** it must produce a summary report containing:
  * Total questions evaluated
  * Mean context recall
  * Mean faithfulness score
  * Mean answer correctness
  * List of failed questions (below threshold on any metric)
* **And** the report must be written to both stdout (human-readable) and a JSON file (machine-readable).

* **Technical Metric:** Evaluation of a 50-question dataset must complete in under 5 minutes (excluding LLM latency).
* **Observability:** Each evaluation run must log `run_id`, `dataset_path`, `model_config`, and aggregate scores via `tracing`.

## Technical Context
* **Architectural patterns:** Command pattern — the evaluator is a standalone binary or CLI subcommand that reuses the existing `RetrievalService` and `Embedder` ports.
* **Stack components:**
    * **Evaluation dataset:** JSONL format (one entry per line) stored in `collections/eval/`.
    * **Metrics:** Context recall (set intersection), cosine similarity (via existing `Embedder`), LLM-as-judge (via existing `LlmClient`).
    * **Report output:** `evaluation_report.json` in the working directory.
* **Integration points:** `RetrievalService::query()` (retrieval + generation), `Embedder::embed()` (similarity), `LlmClient::complete()` (faithfulness judge).
* **Namespace/Config:** `AppConfig::EvalSettings { faithfulness_threshold, correctness_threshold, dataset_path }`.

## Cross-Language Mapping
* Context Recall ≈ `ragas.context_recall` (Python/RAGAS)
* Faithfulness ≈ `ragas.faithfulness` (Python/RAGAS)
* Answer Correctness ≈ `ragas.answer_similarity` (Python/RAGAS)
* JSONL eval dataset ≈ `HuggingFace Dataset` / `OpenAI Evals` format

## Metadata
* **Dependencies:** US-002 (token budget validation — needed for reliable query execution during eval)
* **Complexity:** Medium
* **Reasoning:** The core loop (query → compare → score) is straightforward. Complexity comes from the LLM-as-judge faithfulness check (requires a well-crafted judge prompt) and ensuring the evaluation harness is decoupled enough to run against different configs without recompilation.

## Quality Benchmarks
## Test-First Development Plan
- [ ] Define `EvalEntry` struct and JSONL parser with validation tests.
- [ ] Generate failing test: `given_eval_dataset_when_parsed_then_all_entries_have_required_fields`.
- [ ] Generate failing test: `given_known_retrieval_when_evaluated_then_context_recall_matches_expected`.
- [ ] Generate failing test: `given_grounded_answer_when_judged_then_faithfulness_above_threshold`.
- [ ] Generate failing test: `given_correct_answer_when_compared_then_similarity_above_threshold`.
- [ ] Implement `EvalRunner` service that orchestrates the loop.
- [ ] Implement context recall metric (set intersection on page numbers).
- [ ] Implement answer correctness metric (cosine similarity via `Embedder`).
- [ ] Implement faithfulness metric (LLM-as-judge via `LlmClient`).
- [ ] Implement report generation (stdout + JSON).
- [ ] Create a seed evaluation dataset in `collections/eval/sample.jsonl`.
