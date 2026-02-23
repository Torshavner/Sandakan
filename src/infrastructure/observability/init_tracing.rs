use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

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
                .init();
        }
    }

    tracing::info!(
        port,
        environment = %config.environment,
        json_format = config.json_format,
        otel_enabled = otel_provider.is_some(),
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
