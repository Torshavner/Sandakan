# Health Check
# Verifies the API server is running and healthy.

GET {{base_url}}/health
HTTP 200
[Asserts]
jsonpath "$.status" == "healthy"
