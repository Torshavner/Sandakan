# Error Cases
# Validates that the API returns proper errors for invalid requests.

# Missing file in ingest

POST {{base_url}}/api/v1/ingest
Content-Type: multipart/form-data; boundary=----boundary
HTTP 400
[Asserts]
jsonpath "$.error" isString

# Empty question in query

POST {{base_url}}/api/v1/query
Content-Type: application/json
{
    "question": ""
}
HTTP *
[Asserts]
jsonpath "$.error" isString

# Empty messages in chat completions

POST {{base_url}}/v1/chat/completions
Content-Type: application/json
{
    "model": "rag-pipeline",
    "messages": [],
    "stream": false
}
HTTP 400
[Asserts]
jsonpath "$.error.message" == "No user message provided"
jsonpath "$.error.type" == "invalid_request_error"

# Unsupported content type in ingest

POST {{base_url}}/api/v1/ingest
[MultipartFormData]
file: file,sample-notes.txt; application/xml
HTTP 415
[Asserts]
jsonpath "$.error" contains "Unsupported content type"
