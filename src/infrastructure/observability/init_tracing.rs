// @AI-BYPASS-LENGTH
use std::net::UdpSocket;
use std::sync::Arc;

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
use tracing::Subscriber;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

use super::TracingConfig;

pub fn init_tracing(config: TracingConfig, port: u16) -> Option<SdkTracerProvider> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sandakan=debug,tower_http=debug"));

    let otel_provider = match build_otel_provider(&config) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[sandakan] WARN: OTel layer init failed: {e}");
            None
        }
    };

    let udp_layer = config.udp_endpoint.as_deref().and_then(build_udp_layer);

    match (&config.json_format, &otel_provider) {
        (true, Some(provider)) => {
            let tracer = provider.tracer("sandakan");
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .with(udp_layer)
                .init();
        }
        (false, Some(provider)) => {
            let tracer = provider.tracer("sandakan");
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .with(udp_layer)
                .init();
        }
        (true, None) => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .with(udp_layer)
                .init();
        }
        (false, None) => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .with(udp_layer)
                .init();
        }
    }

    tracing::info!(
        port,
        environment = %config.environment,
        json_format = config.json_format,
        otel_enabled = otel_provider.is_some(),
        udp_enabled = config.udp_endpoint.is_some(),
        "Server initialized"
    );

    otel_provider
}

fn build_otel_provider(config: &TracingConfig) -> anyhow::Result<Option<SdkTracerProvider>> {
    let Some(endpoint) = config.tempo_endpoint.clone() else {
        return Ok(None);
    };

    let resource = Resource::builder()
        .with_attribute(KeyValue::new("service.name", "sandakan"))
        .with_attribute(KeyValue::new(
            "deployment.environment",
            config.environment.clone(),
        ))
        .build();

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource)
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());

    Ok(Some(provider))
}

fn build_udp_layer(addr: &str) -> Option<UdpLayer> {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[sandakan] WARN: UDP log socket bind failed: {e}");
            return None;
        }
    };
    if let Err(e) = socket.connect(addr) {
        eprintln!("[sandakan] WARN: UDP log socket connect to {addr} failed: {e}");
        return None;
    }
    if let Err(e) = socket.set_nonblocking(true) {
        eprintln!("[sandakan] WARN: UDP log socket set_nonblocking failed: {e}");
        return None;
    }
    Some(UdpLayer {
        socket: Arc::new(socket),
    })
}

struct UdpLayer {
    socket: Arc<UdpSocket>,
}

impl<S: Subscriber> Layer<S> for UdpLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        let meta = event.metadata();
        let payload = serde_json::json!({
            "level": meta.level().as_str(),
            "target": meta.target(),
            "file": meta.file(),
            "line": meta.line(),
            "fields": visitor.fields,
        });

        if let Ok(mut bytes) = serde_json::to_vec(&payload) {
            bytes.push(b'\n');
            let _ = self.socket.send(&bytes);
        }
    }
}

#[derive(Default)]
struct JsonVisitor {
    fields: serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for JsonVisitor {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::from(value.to_string()),
        );
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::from(format!("{value:?}")),
        );
    }
}
