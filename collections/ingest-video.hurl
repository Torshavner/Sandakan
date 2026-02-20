# Ingest Video (MP4)
# Uploads an MP4 video file for audio extraction, transcription, and ingestion.
# The API extracts the audio track, transcribes it via Whisper, then embeds.

POST {{base_url}}/api/v1/ingest-reference
Content-Type: application/json
{
    "storage_path": "db-for-gen-ai.mp4",
    "filename": "db-for-gen-ai.mp4",
    "content_type": "video/mp4"
}
HTTP 202
[Captures]
document_id: jsonpath "$.document_id"
job_id: jsonpath "$.job_id"
[Asserts]
jsonpath "$.document_id" isString
jsonpath "$.job_id" isString
jsonpath "$.message" == "Document ingestion started"
