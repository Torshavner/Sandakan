# Ingest Video (MP4)
# Uploads an MP4 video file for audio extraction, transcription, and ingestion.
# The API extracts the audio track, transcribes it via Whisper, then embeds.

POST {{base_url}}/api/v1/ingest
[MultipartFormData]
file: file,sample-video.mp4; video/mp4
HTTP 202
[Captures]
document_id: jsonpath "$.document_id"
job_id: jsonpath "$.job_id"
[Asserts]
jsonpath "$.document_id" isString
jsonpath "$.job_id" isString
jsonpath "$.message" == "Document ingestion started"
