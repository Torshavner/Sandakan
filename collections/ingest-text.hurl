# Ingest Plain Text
# Uploads a plain text document for ingestion.

POST {{base_url}}/api/v1/ingest
[MultipartFormData]
file: file,sample-notes.txt; text/plain
HTTP 202
[Captures]
document_id: jsonpath "$.document_id"
job_id: jsonpath "$.job_id"
[Asserts]
jsonpath "$.document_id" isString
jsonpath "$.job_id" isString
jsonpath "$.message" == "Document ingestion started"
