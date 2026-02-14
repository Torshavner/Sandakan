# Chat Completions (non-streaming)
# Tests the OpenAI-compatible chat completions endpoint.

POST {{base_url}}/v1/chat/completions
Content-Type: application/json
{
    "model": "rag-pipeline",
    "messages": [
        {"role": "user", "content": "What are the main API endpoints?"}
    ],
    "stream": false,
    "temperature": 0.7,
    "max_tokens": 1024
}
HTTP 200
[Asserts]
jsonpath "$.id" startsWith "chatcmpl-"
jsonpath "$.object" == "chat.completion"
jsonpath "$.model" == "rag-pipeline"
jsonpath "$.choices" count == 1
jsonpath "$.choices[0].index" == 0
jsonpath "$.choices[0].message.role" == "assistant"
jsonpath "$.choices[0].message.content" isString
jsonpath "$.choices[0].finish_reason" == "stop"
jsonpath "$.usage.prompt_tokens" isInteger
jsonpath "$.usage.completion_tokens" isInteger
jsonpath "$.usage.total_tokens" isInteger

# Alternative path

POST {{base_url}}/api/chat/completions
Content-Type: application/json
{
    "model": "rag-pipeline",
    "messages": [
        {"role": "user", "content": "Describe the architecture layers."}
    ],
    "stream": false
}
HTTP 200
[Asserts]
jsonpath "$.object" == "chat.completion"
jsonpath "$.choices[0].message.role" == "assistant"
