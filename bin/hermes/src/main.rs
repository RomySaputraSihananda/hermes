use std::time::Duration;

use anyhow::Context;
use engine::{EngineConfig, EngineOutcome};
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mt5_base_url  = std::env::var("MT5_BASE_URL").context("MT5_BASE_URL missing")?;
    std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY missing")?;
    let llm_model     = std::env::var("LLM_MODEL").context("LLM_MODEL missing")?;
    let symbols_raw   = std::env::var("SYMBOLS").context("SYMBOLS missing")?;
    let timeframe_str = std::env::var("TIMEFRAME").context("TIMEFRAME missing")?;
    let candle_count  = std::env::var("CANDLE_COUNT")
        .context("CANDLE_COUNT missing")?
        .parse::<u32>()
        .context("CANDLE_COUNT must be a u32")?;
    let risk_pct      = std::env::var("RISK_PCT")
        .context("RISK_PCT missing")?
        .parse::<Decimal>()
        .context("RISK_PCT must be a decimal (e.g. 0.01)")?;
    let min_sl_distance = std::env::var("MIN_SL_DISTANCE")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<Decimal>()
        .context("MIN_SL_DISTANCE must be a decimal (e.g. 0.0010)")?;
    if min_sl_distance < Decimal::ZERO {
        anyhow::bail!("MIN_SL_DISTANCE must be >= 0, got {min_sl_distance}");
    }
    let cycle_secs    = std::env::var("CYCLE_SECS")
        .context("CYCLE_SECS missing")?
        .parse::<u64>()
        .context("CYCLE_SECS must be a u64")?;

    let timeframe = timeframe_str
        .parse::<domain::Timeframe>()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let symbols: Vec<String> = symbols_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if symbols.is_empty() {
        anyhow::bail!("SYMBOLS must contain at least one symbol");
    }

    let mt5    = mt5_client::Mt5Client::new(mt5_base_url);
    let llm    = openrouter::OpenRouterClient::new();
    let config = EngineConfig { timeframe, candle_count, risk_pct, min_sl_distance };

    tracing::info!(
        symbols    = ?symbols,
        timeframe  = ?config.timeframe,
        candle_count = config.candle_count,
        risk_pct   = %config.risk_pct,
        min_sl_distance = %config.min_sl_distance,
        cycle_secs,
        "hermes starting"
    );

    loop {
        let start = tokio::time::Instant::now();

        let symbol_refs: Vec<&str> = symbols.iter().map(String::as_str).collect();

        match engine::run_once(&symbol_refs, &mt5, &llm, &llm_model, &config).await {
            Ok(EngineOutcome::Traded { symbol, action, volume, order }) => {
                tracing::debug!(symbol, ?action, %volume, order, "trade confirmed");
            }
            Ok(EngineOutcome::NoSignal)   => tracing::debug!("no ICT signal on any symbol"),
            Ok(EngineOutcome::Hold)       => tracing::debug!("agents voted hold on all symbols"),
            Ok(EngineOutcome::NoApproval) => tracing::debug!("risk rejected all candidates"),
            Err(e) => tracing::error!(error = %e, "cycle error, continuing"),
        }

        let elapsed = start.elapsed();
        let cycle   = Duration::from_secs(cycle_secs);
        if elapsed < cycle {
            tokio::time::sleep(cycle - elapsed).await;
        }
    }
}
