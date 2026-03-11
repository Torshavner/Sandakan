# Query RAG
# Sends a question to the RAG pipeline and validates the response shape.
# Requires documents to have been ingested first.

# "What is the difference between standard retrieval and the ColPali approach?",
# "What are the production Do's for RAG?"
# "Why is hybrid search better than vector-only search?",

POST {{base_url}}/api/v1/query
Content-Type: application/json
{
    "question": "Tell me more about ColPali revolution in terms of VLMs"
}
HTTP 200
[Asserts]
jsonpath "$.answer" isString
jsonpath "$.sources" isCollection