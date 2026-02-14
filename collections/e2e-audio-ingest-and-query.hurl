# End-to-End: Ingest Audio then Query
# Full workflow: upload an audio file, poll job status until completed, then query.

# Step 1: Verify server is healthy

GET {{base_url}}/health
HTTP 200
[Asserts]
jsonpath "$.status" == "healthy"

# Step 2: Ingest an audio file

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

# Step 3: Poll job status (initial check — should be queued or processing)

GET {{base_url}}/api/v1/jobs/{{job_id}}
HTTP 200
[Asserts]
jsonpath "$.id" == {{job_id}}
jsonpath "$.status" isString
jsonpath "$.job_type" == "document_ingestion"

# Step 4: Query for content from the transcribed audio
# NOTE: In a real e2e run, add a delay or retry loop between step 3 and 4
# to wait for the job to reach COMPLETED status.

POST {{base_url}}/api/v1/query
Content-Type: application/json
{
    "question": "What topics are discussed in the audio recording?"
}
HTTP 200
[Asserts]
jsonpath "$.answer" isString
jsonpath "$.sources" isCollection

# Step 5: Ask a follow-up via chat completions

POST {{base_url}}/v1/chat/completions
Content-Type: application/json
{
    "model": "rag-pipeline",
    "messages": [
        {"role": "user", "content": "Summarize the key points from the audio transcript."}
    ],
    "stream": false
}
HTTP 200
[Asserts]
jsonpath "$.object" == "chat.completion"
jsonpath "$.choices[0].message.content" isString
