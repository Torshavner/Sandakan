# Middleware: Distributed Tracing & Correlation ID Injection

## Requirement Definition

As a System Operator, I need a middleware layer that injects or propagates a unique Correlation ID for every incoming HTTP request so that I can trace the entire lifecycle of an Agentic workflow across asynchronous task boundaries, vector database calls, and LLM inferences.

## Problem Statement

* **Current bottleneck/technical debt:** Asynchronous `tokio` tasks interleave logs in standard output. When an agent fails during a multi-step tool-calling sequence, it is nearly impossible to isolate which log lines belong to the failing request.
* **Performance/cost implications:** Without distributed tracing, debugging complex RAG workflows takes exponentially longer, delaying bug fixes and hindering system evaluation.
* **Architectural necessity:** To evaluate the agent effectively (as required by the assignment), we must be able to observe the exact path a specific prompt took through the system.

## Acceptance Criteria (Gherkin Enforced)

### ID Generation & Injection

* **Given** an incoming HTTP request to the Agent API,
* **When** the request does not contain an `x-correlation-id` header,
* **Then** the middleware must generate a standard UUIDv4, attach it to the request extensions, and inject it into the root `tracing` span.

### Header Propagation

* **Given** an incoming HTTP request containing an `x-correlation-id` header from an upstream client,
* **When** the middleware processes the request,
* **Then** it must sanitize and adopt the provided ID rather than generating a new one, ensuring end-to-end traceability.

### Response Tagging

* **Given** a successfully processed or failed request,
* **When** the HTTP response is dispatched to the client,
* **Then** the response headers must include the `x-correlation-id` so the end-user or UI can reference it in bug reports.

## Technical Context

* **Architectural patterns:** Intercepting Filter / Middleware.
* **Stack components:** Rust, `tower` / `tower-http`, `tracing`, `tracing-subscriber`.
* **Integration points:** API Router, Logger configuration.

## Test-First Development Plan

* [ ] Create a mock endpoint that logs a message using the `tracing` macro.
* [ ] Write a test sending a request *without* the header and assert the response contains a generated UUID header.
* [ ] Write a test sending a request *with* `x-correlation-id: custom-test-123` and assert the logs and response reflect `custom-test-123`.

---

# Middleware: Client Disconnect & Async Task Cancellation

## Requirement Definition

As an API Designer, I need the server to immediately halt processing if the client drops the connection so that we do not waste expensive GPU compute, OpenAI credits, or Qdrant connections on orphaned requests.

## Problem Statement

* **Current bottleneck/technical debt:** RAG searches and LLM text generation can take 5–15 seconds. If a user navigates away from the page or repeatedly hits "refresh," the server continues to process the abandoned requests in the background.
* **Performance/cost implications:** Orphaned tasks quickly exhaust rate limits, drain token budgets, and lock up local compute resources, leading to cascading system degradation.
* **Architectural necessity:** In Rust, asynchronous futures do nothing unless polled. We must ensure the HTTP framework drops the future when the TCP socket closes.

## Acceptance Criteria (Gherkin Enforced)

### Context Cancellation

* **Given** a long-running Agentic task (e.g., waiting for an OpenAI response),
* **When** the client abruptly closes the TCP connection or the browser tab,
* **Then** the middleware must detect the broken pipe and drop the associated `tokio` future.

### Resource Cleanup

* **Given** a cancelled request future,
* **When** the `Drop` trait is executed,
* **Then** any active outbound network streams (e.g., to Qdrant or the LLM) must be immediately aborted without completing.

## Technical Context

* **Architectural patterns:** Circuit Breaker / Context Cancellation.
* **Stack components:** Rust, `tokio::select!`, Axum/Actix cancellation tokens.
* **Integration points:** Global HTTP middleware stack, Streaming response handlers.

## Test-First Development Plan

* [ ] Create a dummy endpoint containing a `tokio::time::sleep(Duration::from_secs(10))`.
* [ ] Write an integration test that connects, starts the request, and forcefully drops the connection after 1 second.
* [ ] Assert via atomic counters or logs that the code *after* the sleep was never executed.

---

# Middleware: Bearer Token Authentication

## Requirement Definition

As a Security Architect, I need to protect the Agent API behind a pre-shared API key authorization layer so that unauthorized users or automated internal network scanners cannot trigger expensive LLM workflows.

## Problem Statement

* **Current bottleneck/technical debt:** The API is currently open to the internal network. Using a spoofable header like `authorized: true` provides zero cryptographic or practical security against misuse.
* **Performance/cost implications:** Unauthorized access could lead to rapid depletion of API quotas, exposing the project to unexpected billing spikes from OpenAI or local infrastructure overload.
* **Architectural necessity:** Even internal proofs-of-concept require a basic layer of defense-in-depth to establish safe software development practices.

## Acceptance Criteria (Gherkin Enforced)

### Valid Token Access

* **Given** an incoming request to a protected Agent endpoint,
* **When** the request includes an `Authorization: Bearer <VALID_SECRET>` header,
* **Then** the middleware must strip the header, validate the secret against the environment configuration, and allow the request to proceed.

### Missing or Malformed Token

* **Given** an incoming request,
* **When** the `Authorization` header is entirely missing, or does not follow the `Bearer <token>` format,
* **Then** the middleware must intercept the request and immediately return an HTTP `401 Unauthorized` status code.

### Invalid Token Rejection

* **Given** an incoming request with a correctly formatted Bearer token,
* **When** the token string does not match the securely stored application secret,
* **Then** the middleware must log a security warning (including the Correlation ID) and return an HTTP `403 Forbidden` or `401 Unauthorized`.

## Technical Context

* **Architectural patterns:** API Gateway / Authentication Filter.
* **Stack components:** Rust, Web Framework extractors (e.g., `axum-auth` or custom `tower` layer).
* **Integration points:** Environment variables (`.env`), API routing tree.

## Test-First Development Plan

* [ ] Configure the test environment with `AGENT_API_KEY=test_secret_123`.
* [ ] Assert that requests with no header return `401 Unauthorized`.
* [ ] Assert that requests with `Authorization: Bearer wrong_key` return `401 Unauthorized`.
* [ ] Assert that requests with `Authorization: Bearer test_secret_123` return `200 OK` (or the intended endpoint success code).
