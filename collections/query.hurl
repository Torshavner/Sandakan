# Query RAG
# Sends a question to the RAG pipeline and validates the response shape.
# Requires documents to have been ingested first.

POST {{base_url}}/api/v1/query
Content-Type: application/json
{
    "question": "What is the Sandakan RAG pipeline?"
}
HTTP 200
[Asserts]
jsonpath "$.answer" isString
jsonpath "$.sources" isCollection

# Query with conversation_id to test multi-turn

POST {{base_url}}/api/v1/query
Content-Type: application/json
{
    "question": "What architecture does it use?",
    "conversation_id": "{{conversation_id}}"
}
HTTP 200
[Asserts]
jsonpath "$.answer" isString
jsonpath "$.sources" isCollection
