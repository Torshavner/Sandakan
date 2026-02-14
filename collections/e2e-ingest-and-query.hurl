# End-to-End: Ingest PDF then Query
# Full workflow: upload a PDF, wait briefly, then query for its content.

# Step 1: Verify server is healthy

GET {{base_url}}/health
HTTP 200
[Asserts]
jsonpath "$.status" == "healthy"

# Step 2: Ingest the sample PDF

POST {{base_url}}/api/v1/ingest
[MultipartFormData]
file: file,sample-rag-docs.pdf; application/pdf
HTTP 202
[Captures]
document_id: jsonpath "$.document_id"
[Asserts]
jsonpath "$.document_id" isString
jsonpath "$.message" == "Document ingestion started"

# Step 3: Query for content from the ingested document

POST {{base_url}}/api/v1/query
Content-Type: application/json
{
    "question": "What is the Sandakan RAG pipeline and what architecture does it use?"
}
HTTP 200
[Asserts]
jsonpath "$.answer" isString
jsonpath "$.sources" isCollection

# Step 4: Ask a follow-up via chat completions

POST {{base_url}}/v1/chat/completions
Content-Type: application/json
{
    "model": "rag-pipeline",
    "messages": [
        {"role": "user", "content": "What are the four architecture layers in Sandakan?"}
    ],
    "stream": false
}
HTTP 200
[Asserts]
jsonpath "$.object" == "chat.completion"
jsonpath "$.choices[0].message.content" isString
