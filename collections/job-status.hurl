# Job Status
# Polls the job status endpoint after submitting an ingestion job.
# First ingests a file, captures the job_id, then queries the status.

# Step 1: Ingest a file to get a job_id

POST {{base_url}}/api/v1/ingest
[MultipartFormData]
file: file,sample-rag-docs.pdf; application/pdf
HTTP 202
[Captures]
job_id: jsonpath "$.job_id"

# Step 2: Poll job status

GET {{base_url}}/api/v1/jobs/{{job_id}}
HTTP 200
[Asserts]
jsonpath "$.id" == {{job_id}}
jsonpath "$.status" isString
jsonpath "$.document_id" isString
jsonpath "$.job_type" == "document_ingestion"
jsonpath "$.created_at" isString
jsonpath "$.updated_at" isString
