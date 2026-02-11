// main.rs
mod handlers;
mod router;
mod settings;
mod logging;

use std::env;
use app_core::coin_pair::CoinPair;
use app_core::exchange_client::ExchangeClient;
use config::{Config, File};
use config::Environment as EnvironmentSource;

use application::pipeline::filters::{VWapAggregator, LeadLagAnalyzer};
use application::pipeline::sinks::{TradeWriter, VwapWriter, LeadLagWriter, state_updater_task};
use application::state::VwapState;

use infrastructure::binance::BinanceClient;
use infrastructure::kraken::KrakenClient;

use app_core::Trade;
use settings::{Settings, Environment};

use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Configuration
    dotenvy::dotenv().ok();

    let environment: Environment = env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let configuration = Config::builder()
        .add_source(
            File::with_name(&format!("appsettings.{}", environment.as_str()))
                .required(false))
        .add_source(
            EnvironmentSource::with_prefix("APP")
                .separator("_")
                .list_separator(" "),
        )
        .build()?;

    let settings: Settings = configuration.try_deserialize()?;

    // Logger
    let enable_udp = environment == Environment::Local;
    logging::init_tracing(enable_udp)?;

    tracing::info!("Tracing initialized. Application starting in {:?} mode.", environment);

    // Migrations
    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(settings.database.url.as_str())
        .await
        .expect("Failed to connect to database");

    tracing::info!("Running database migrations...");

    sqlx::migrate!()
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Migrations complete.");

    // Background tasks
    let (binance_trade_tx, binance_trade_rx_writer) = broadcast::channel::<Trade>(1024);
    let binance_trade_rx_aggregator = binance_trade_tx.subscribe();
    let binance_trade_rx_lead_lag = binance_trade_tx.subscribe();

    let (kraken_trade_tx, kraken_trade_rx_writer) = broadcast::channel::<Trade>(1024);
    let kraken_trade_rx_lead_lag = kraken_trade_tx.subscribe();

    let (vwap_tx, vwap_rx_api) = broadcast::channel(128);
    let vwap_rx_db = vwap_tx.subscribe();

    let (lead_lag_tx, lead_lag_rx) = broadcast::channel(128);

    let binance_client = BinanceClient::new(binance_trade_tx);
    let kraken_client = KrakenClient::new(kraken_trade_tx);

    let binance_trade_writer = TradeWriter::new(
        binance_trade_rx_writer,
        db_pool.clone(),
    );

    let kraken_trade_writer = TradeWriter::new(
        kraken_trade_rx_writer,
        db_pool.clone(),
    );

    let aggregator = VWapAggregator::new(
        CoinPair::new("BTC", "USDT").expect(CoinPair::COIN_PAIR_INVALID),
        binance_trade_rx_aggregator,
        vwap_tx,
        1,
    );

    let vwap_writer = VwapWriter::new(vwap_rx_db, db_pool.clone());

    let lead_lag_analyzer = LeadLagAnalyzer::new(
        binance_trade_rx_lead_lag,
        kraken_trade_rx_lead_lag,
        lead_lag_tx,
    );

    let lead_lag_writer = LeadLagWriter::new(lead_lag_rx, db_pool.clone(), 100);

    let api_state = VwapState::default();

    tracing::info!("Spawning background tasks...");
    tokio::spawn(binance_client.run());
    tokio::spawn(kraken_client.run());
    tokio::spawn(binance_trade_writer.run());
    tokio::spawn(kraken_trade_writer.run());
    tokio::spawn(aggregator.run());
    tokio::spawn(vwap_writer.run());
    tokio::spawn(lead_lag_analyzer.run());
    tokio::spawn(lead_lag_writer.run());
    tokio::spawn(state_updater_task(vwap_rx_api, api_state.clone()));

    // api
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = router::create_router(api_state).layer(cors);

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", settings.host, settings.port))
        .await
        .unwrap();

    tracing::info!(
        "API server listening on http://{}:{}",
        settings.host,
        settings.port
    );

    axum::serve(listener, app).await.unwrap();

    Ok(())
}