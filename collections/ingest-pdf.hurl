# Ingest PDF
# Uploads a PDF document for ingestion into the RAG pipeline.
# The API should accept the file and return 202 with a document_id.

POST {{base_url}}/api/v1/ingest
[MultipartFormData]
file: file,sample-rag-docs.pdf; application/pdf
HTTP 202
[Captures]
document_id: jsonpath "$.document_id"
job_id: jsonpath "$.job_id"
[Asserts]
jsonpath "$.document_id" isString
jsonpath "$.job_id" isString
jsonpath "$.message" == "Document ingestion started"
