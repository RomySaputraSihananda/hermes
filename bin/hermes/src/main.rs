use std::time::Duration;

use anyhow::Context;
use chrono::{Timelike, Utc};
use engine::{EngineConfig, EngineOutcome};
use rust_decimal::Decimal;
use telegram::TelegramConfig;

const MAGIC: u64 = 19731;

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
    let symbols_raw   = std::env::var("SYMBOLS").context("SYMBOLS missing")?;
    let timeframe_str = std::env::var("TIMEFRAME").context("TIMEFRAME missing")?;
    let candle_count  = std::env::var("CANDLE_COUNT")
        .context("CANDLE_COUNT missing")?
        .parse::<u32>()
        .context("CANDLE_COUNT must be a u32")?;
    let risk_pct = std::env::var("RISK_PCT")
        .context("RISK_PCT missing")?
        .parse::<Decimal>()
        .context("RISK_PCT must be a decimal (e.g. 0.01)")?;
    let min_sl_distance = std::env::var("MIN_SL_DISTANCE")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<Decimal>()
        .context("MIN_SL_DISTANCE must be a decimal")?;
    let cycle_secs = std::env::var("CYCLE_SECS")
        .context("CYCLE_SECS missing")?
        .parse::<u64>()
        .context("CYCLE_SECS must be a u64")?;
    let summary_hour: Option<u32> = std::env::var("SUMMARY_HOUR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().context("SUMMARY_HOUR must be 0-23"))
        .transpose()?;

    if min_sl_distance < Decimal::ZERO {
        anyhow::bail!("MIN_SL_DISTANCE must be >= 0");
    }

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

    // ── Telegram (optional) ──────────────────────────────────────────────────
    let tg_token    = std::env::var("TELEGRAM_BOT_TOKEN").ok().filter(|s| !s.is_empty());
    let tg_chat_id  = std::env::var("TELEGRAM_CHAT_ID").ok()
        .and_then(|s| s.parse::<i64>().ok());
    let tg_thread_id = std::env::var("TELEGRAM_THREAD_ID").ok()
        .and_then(|s| s.parse::<i64>().ok());

    let tg: Option<TelegramConfig> = match (tg_token, tg_chat_id) {
        (Some(token), Some(chat_id)) => Some(TelegramConfig { token, chat_id, thread_id: tg_thread_id }),
        _ => None,
    };

    let http = reqwest::Client::new();
    let mt5  = mt5_client::Mt5Client::new(mt5_base_url.clone());

    let config = EngineConfig { timeframe, candle_count, risk_pct, min_sl_distance };

    tracing::info!(
        symbols    = ?symbols,
        timeframe  = ?config.timeframe,
        candle_count = config.candle_count,
        risk_pct   = %config.risk_pct,
        cycle_secs,
        "hermes starting"
    );

    // ── Startup notification ─────────────────────────────────────────────────
    if let Some(tg) = &tg {
        let text = format!(
            "⚡ <b>HERMES started</b>\n\nSymbols  <code>{}</code>\nRisk     {}%\nTimeframe  {}\nCycle    {}s\nMAGIC    {}",
            symbols.join(", "),
            risk_pct * Decimal::from(100),
            timeframe_str,
            cycle_secs,
            MAGIC,
        );
        if let Err(e) = tg.send(&http, &text).await {
            tracing::warn!("Telegram startup failed: {e:#}");
        }
    }

    let mut last_summary_hour: Option<u32> = None;

    loop {
        let start = tokio::time::Instant::now();
        let now   = Utc::now();

        // ── Daily PnL summary (once per hour match) ──────────────────────────
        if let (Some(target_hour), Some(tg)) = (summary_hour, &tg) {
            let current_hour = now.hour();
            if current_hour == target_hour && last_summary_hour != Some(current_hour) {
                last_summary_hour = Some(current_hour);
                if let Err(e) = send_pnl_summary(&mt5, &http, &symbols, tg).await {
                    tracing::warn!("PnL summary failed: {e:#}");
                }
            } else if current_hour != target_hour {
                last_summary_hour = None; // reset so it fires again next day
            }
        }

        // ── Trading cycle ────────────────────────────────────────────────────
        let symbol_refs: Vec<&str> = symbols.iter().map(String::as_str).collect();

        match engine::run_once(&symbol_refs, &mt5, &config).await {
            Ok(EngineOutcome::Traded { symbol, volume, order, side, entry, sl, tp }) => {
                tracing::info!(%symbol, ?side, %volume, order, %entry, %sl, %tp, "trade executed");

                if let Some(tg) = &tg {
                    let side_str = format!("{side:?}");
                    let rr = if entry != sl {
                        let risk_dist = (entry - sl).abs();
                        let rew_dist  = (tp - entry).abs();
                        format!("{:.1}", rew_dist / risk_dist)
                    } else {
                        "?".to_string()
                    };
                    let text = format!(
                        "🟡 <b>TRADE</b>\n{} · {}\n\nEntry  <code>{}</code>\nTP     <code>{}</code>\nSL     <code>{}</code>\nVol {} lot  ·  RR 1:{}",
                        symbol, side_str,
                        fmt_price(entry), fmt_price(tp), fmt_price(sl),
                        volume, rr,
                    );
                    if let Err(e) = tg.send(&http, &text).await {
                        tracing::warn!("Telegram trade notify failed: {e:#}");
                    }
                }
            }
            Ok(EngineOutcome::NoSignal)   => tracing::debug!("no ICT signal on any symbol"),
            Ok(EngineOutcome::NoApproval) => tracing::debug!("risk rejected all candidates"),
            Err(e) => {
                let msg = e.to_string();
                tracing::error!(error = %msg, "cycle error, continuing");
                let market_closed = msg.contains("10018") || msg.to_lowercase().contains("market closed");
                if !market_closed {
                    if let Some(tg) = &tg {
                        let text = format!("⚠️ <b>HERMES error</b>\n<code>{msg}</code>");
                        let _ = tg.send(&http, &text).await;
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        let cycle   = Duration::from_secs(cycle_secs);
        if elapsed < cycle {
            tokio::time::sleep(cycle - elapsed).await;
        }
    }
}

// ── PnL summary ───────────────────────────────────────────────────────────────

async fn send_pnl_summary(
    mt5:     &mt5_client::Mt5Client,
    http:    &reqwest::Client,
    symbols: &[String],
    tg:      &TelegramConfig,
) -> anyhow::Result<()> {
    let now       = Utc::now();
    let today_str = now.format("%Y-%m-%dT00:00:00").to_string();
    let now_str   = now.format("%Y-%m-%dT%H:%M:%S").to_string();
    let epoch_str = "2020-01-01T00:00:00".to_string();

    let sym_opt = if symbols.len() == 1 { Some(symbols[0].as_str()) } else { None };

    let (today_deals, all_deals) = tokio::try_join!(
        mt5.history_deals(&today_str, &now_str, sym_opt),
        mt5.history_deals(&epoch_str, &now_str, sym_opt),
    )?;

    let account = mt5.account().await?;

    let summarize = |deals: &[mt5_client::Deal]| {
        let closing: Vec<_> = deals.iter()
            .filter(|d| d.entry == 1 && d.magic == MAGIC)
            .collect();
        let total  = closing.len();
        let wins   = closing.iter().filter(|d| d.profit > Decimal::ZERO).count();
        let losses = total - wins;
        let net: Decimal = closing.iter().map(|d| d.profit + d.commission + d.swap).sum();
        let wr = if total > 0 { wins as f64 / total as f64 * 100.0 } else { 0.0 };
        format!(
            "{total} trades  ·  {wins}W {losses}L  ·  WR {wr:.0}%\nNet  <b>{net:+.2}</b>",
        )
    };

    let today_date   = now.format("%Y-%m-%d").to_string();
    let sym_label    = symbols.join(", ");
    let balance      = account.balance;

    let text = format!(
        "📊 <b>PnL Summary</b>\n\n<b>Today</b>  ·  {}  ·  {}\n{}\n\n<b>All-time</b>\n{}\n\nBalance  <b>${balance:.2}</b>",
        sym_label, today_date,
        summarize(&today_deals),
        summarize(&all_deals),
    );

    tg.send(http, &text).await?;
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn fmt_price(v: Decimal) -> String {
    let s = format!("{v:.5}");
    let s = s.trim_end_matches('0');
    s.trim_end_matches('.').to_string()
}
