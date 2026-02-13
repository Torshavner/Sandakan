# User Story Generation Specification

## File Naming Convention

`docs/user_stories/{id}_{feature_name_snake_case}.md`

---

## Structural Schema

### Header

# {Feature Name}

### Requirement Definition

As a {role}, I need {capability} so that {business_value}.

### Problem Statement

* Current bottleneck/technical debt
* Performance/cost implications
* Architectural necessity

### Acceptance Criteria (Gherkin Enforced)

#### {Logical Group}

* Given {state}, when {action}, then {outcome}
* {Technical_metric_requirement}
* {Observability_requirement}

### Technical Context

* Architectural patterns/stack components
* Integration points
* Namespace/Config references

### Cross-Language Mapping

* {Primary_concept} ≈ {Secondary_language_equivalent}

### Metadata

* Dependencies: {ID_list | None}
* Complexity: {Low | Medium | High}
* Reasoning: {Complexity_drivers if High}

---

## Quality Benchmarks

### Semantic Precision

* Avoid vague terms (fast, reliable, better).
* Quantify performance (p99 < Xms, throughput > Y req/s).
* Define specific error codes and log levels.

### Testability

* Criteria must map 1:1 to test functions.
* Naming convention: `given_{state}__when_{action}__then_{outcome}`.

### Behavioral Focus

* Define "what" not "how" in criteria.
* Reserve implementation details for Technical Context.

---

## Workflow: Test-First Development

1. Parse criteria into Given-When-Then scenarios.
2. Generate failing test suite.
3. Execute `test` command to confirm failure.
4. Implement minimal logic for pass.
5. Refactor under green state.

---

## Indexing Protocol

Append to `INDEX.md`:
`{ID}. {Title} -> {Path}`

* Keywords: {tags}
* Deps: {ID}
* Complexity: {Rating}

**Next Step:** Provide a feature description to generate an optimized User Story.