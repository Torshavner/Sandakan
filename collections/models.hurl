# List Models
# Verifies the /v1/models endpoint returns the RAG pipeline model.

GET {{base_url}}/v1/models
HTTP 200
[Asserts]
jsonpath "$.object" == "list"
jsonpath "$.data" count == 1
jsonpath "$.data[0].id" == "rag-pipeline"
jsonpath "$.data[0].object" == "model"
jsonpath "$.data[0].owned_by" == "local"

# Alternative path

GET {{base_url}}/api/models
HTTP 200
[Asserts]
jsonpath "$.object" == "list"
jsonpath "$.data[0].id" == "rag-pipeline"
