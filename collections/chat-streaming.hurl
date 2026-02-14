# Chat Completions (streaming)
# Tests the SSE streaming chat completions endpoint.
# Validates the response is an event stream.

POST {{base_url}}/v1/chat/completions
Content-Type: application/json
{
    "model": "rag-pipeline",
    "messages": [
        {"role": "user", "content": "What layers does the architecture have?"}
    ],
    "stream": true
}
HTTP 200
[Asserts]
header "Content-Type" contains "text/event-stream"
