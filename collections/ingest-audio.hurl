# Ingest Audio
# Uploads an audio file for transcription and ingestion into the RAG pipeline.
# The API accepts the file and returns 202 with document_id and job_id.

POST {{base_url}}/api/v1/ingest
[MultipartFormData]
file: file,sample-audio.mp3; audio/mpeg
HTTP 202
[Captures]
document_id: jsonpath "$.document_id"
job_id: jsonpath "$.job_id"
[Asserts]
jsonpath "$.document_id" isString
jsonpath "$.job_id" isString
jsonpath "$.message" == "Document ingestion started"
