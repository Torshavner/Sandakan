use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// Newtype wrapper so handlers can extract the correlation ID from request extensions.
#[derive(Clone, Debug)]
pub struct CorrelationId(pub String);

/// Propagates or generates a `x-correlation-id` for every incoming request.
///
/// Priority: adopt the upstream-supplied header when present; generate a UUIDv4 otherwise.
/// The ID is attached to the request extensions and echoed back in the response header.
pub async fn correlation_id_middleware(mut request: Request, next: Next) -> Response {
    let correlation_id = request
        .headers()
        .get(CORRELATION_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    request
        .extensions_mut()
        .insert(CorrelationId(correlation_id.clone()));

    let span = tracing::info_span!(
        "correlation",
        correlation_id = %correlation_id,
    );

    let mut response = next.run(request).instrument(span).await;

    if let Ok(header_value) = HeaderValue::from_str(&correlation_id) {
        response
            .headers_mut()
            .insert(CORRELATION_ID_HEADER, header_value);
    }

    response
}
