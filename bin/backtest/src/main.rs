use anyhow::Context;
use domain::Side;
use ict::IctAnalyzer;
use rust_decimal::Decimal;

struct SimTrade {
    open_time_str: String,
    signal:        ict::TradeSignal,
    volume:        Decimal,
}

fn make_sim_account(balance: Decimal) -> domain::AccountInfo {
    domain::AccountInfo {
        login:           0,
        leverage:        100,
        trade_allowed:   true,
        trade_expert:    true,
        currency:        "USD".to_string(),
        currency_digits: 2,
        server:          "sim".to_string(),
        name:            "sim".to_string(),
        company:         "sim".to_string(),
        balance,
        equity:          balance,
        profit:          Decimal::ZERO,
        credit:          Decimal::ZERO,
        margin:          Decimal::ZERO,
        margin_free:     balance,
        margin_level:    Decimal::ZERO,
    }
}

fn fmt_price(d: Decimal, prec: usize) -> String {
    format!("{0:.1$}", d, prec)
}

fn fmt_pnl(pnl: Decimal) -> String {
    if pnl >= Decimal::ZERO {
        format!("+{:.2}", pnl)
    } else {
        format!("{:.2}", pnl)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mt5_base_url     = std::env::var("MT5_BASE_URL").context("MT5_BASE_URL missing")?;
    std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY missing")?;
    let llm_model        = std::env::var("LLM_MODEL").context("LLM_MODEL missing")?;
    let symbol           = std::env::var("SYMBOL").context("SYMBOL missing")?;
    let timeframe_str    = std::env::var("TIMEFRAME").context("TIMEFRAME missing")?;
    let window_size      = std::env::var("CANDLE_COUNT")
        .context("CANDLE_COUNT missing")?
        .parse::<usize>()
        .context("CANDLE_COUNT must be a positive integer")?;
    if window_size == 0 {
        anyhow::bail!("CANDLE_COUNT must be at least 1");
    }
    let risk_pct         = std::env::var("RISK_PCT")
        .context("RISK_PCT missing")?
        .parse::<Decimal>()
        .context("RISK_PCT must be a decimal (e.g. 0.01)")?;
    let backtest_candles = std::env::var("BACKTEST_CANDLES")
        .context("BACKTEST_CANDLES missing")?
        .parse::<u32>()
        .context("BACKTEST_CANDLES must be a u32")?;
    let backtest_balance = std::env::var("BACKTEST_BALANCE")
        .context("BACKTEST_BALANCE missing")?
        .parse::<Decimal>()
        .context("BACKTEST_BALANCE must be a decimal (e.g. 5000)")?;
    let min_sl_distance = std::env::var("MIN_SL_DISTANCE")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<Decimal>()
        .context("MIN_SL_DISTANCE must be a decimal (e.g. 0.0010)")?;

    let timeframe = timeframe_str
        .parse::<domain::Timeframe>()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let tf_short = timeframe_str.as_str();

    let mt5 = mt5_client::Mt5Client::new(mt5_base_url);
    let llm = openrouter::OpenRouterClient::new();

    tracing::info!(symbol = %symbol, timeframe = ?timeframe, backtest_candles, "fetching historical data");

    let (symbol_info, candles) = tokio::try_join!(
        mt5.symbol(&symbol),
        mt5.rates_from_pos(&symbol, timeframe, 0, backtest_candles),
    )?;

    let total = candles.len();
    if total <= window_size {
        anyhow::bail!("not enough candles: got {total}, need more than {window_size}");
    }

    let contract_size = symbol_info.trade_contract_size;
    let prec          = symbol_info.digits as usize;

    let risk_cfg = risk::RiskConfig {
        risk_pct,
        value_per_lot: contract_size,
        min_volume:    symbol_info.volume_min,
        max_volume:    symbol_info.volume_max,
        volume_step:   symbol_info.volume_step,
    };

    tracing::info!(total, window_size, "starting walk-forward");

    let mut balance      = backtest_balance;
    let mut account_sim  = make_sim_account(balance);
    let mut open_trade: Option<SimTrade> = None;
    let mut peak         = balance;
    let mut max_drawdown = Decimal::ZERO;

    let mut trades    = 0u32;
    let mut wins      = 0u32;
    let mut losses    = 0u32;
    let mut timeouts  = 0u32;
    let mut total_pnl = Decimal::ZERO;

    for i in 0..(total - window_size) {
        let new_idx    = i + window_size;
        let new_candle = &candles[new_idx];

        // --- check open trade ---
        if open_trade.is_some() {
            let (sl_hit, tp_hit) = {
                let t = open_trade.as_ref().unwrap();
                match t.signal.side {
                    Side::Long  => (
                        new_candle.low  <= t.signal.sl,
                        new_candle.high >= t.signal.tp,
                    ),
                    Side::Short => (
                        new_candle.high >= t.signal.sl,
                        new_candle.low  <= t.signal.tp,
                    ),
                }
            };

            if sl_hit || tp_hit {
                let trade   = open_trade.take().unwrap();
                let sl_wins = sl_hit; // SL takes priority when both hit on same candle
                let (label, exit) = if sl_wins {
                    ("SL     ", trade.signal.sl)
                } else {
                    ("TP     ", trade.signal.tp)
                };
                let pnl = match trade.signal.side {
                    Side::Long  => (exit - trade.signal.entry) * trade.volume * contract_size,
                    Side::Short => (trade.signal.entry - exit) * trade.volume * contract_size,
                };
                balance += pnl;
                if balance > peak { peak = balance; }
                let dd = balance - peak;
                if dd < max_drawdown { max_drawdown = dd; }
                if sl_wins { losses += 1; } else { wins += 1; }
                trades    += 1;
                total_pnl += pnl;
                account_sim = make_sim_account(balance);

                println!(
                    "[{} {}] {} {} entry={} tp={} sl={} vol={:.2} → {} exit={} pnl={} bal={:.2}",
                    trade.open_time_str, tf_short, symbol,
                    if trade.signal.side == Side::Long { "LONG " } else { "SHORT" },
                    fmt_price(trade.signal.entry, prec),
                    fmt_price(trade.signal.tp, prec),
                    fmt_price(trade.signal.sl, prec),
                    trade.volume, label,
                    fmt_price(exit, prec),
                    fmt_pnl(pnl), balance,
                );
            }
            continue; // trade was open (or just closed) — skip new signal on same candle
        }

        // --- no open trade: run pipeline ---
        let window   = &candles[i..new_idx];
        let analysis = IctAnalyzer::new(window, min_sl_distance).analyze();
        let signal   = match analysis.signal.as_ref() {
            Some(s) => s.clone(),
            None    => continue,
        };

        let technical_in   = agents::TechnicalInput   { symbol: &symbol, candles: window, analysis: &analysis };
        let sentiment_in   = agents::SentimentInput   { symbol: &symbol, candles: window };
        let fundamental_in = agents::FundamentalInput { symbol: &symbol, news_context: "" };
        let risk_in        = agents::RiskInput        { account: &account_sim, positions: &[], signal: Some(&signal) };

        let decision = agents::run_agents(&llm, &llm_model, technical_in, sentiment_in, fundamental_in, risk_in).await?;

        if decision.action == agents::Action::Hold {
            continue;
        }

        let risk_dec = risk::evaluate(&account_sim, &signal, &risk_cfg);
        if !risk_dec.approved {
            continue;
        }

        open_trade = Some(SimTrade {
            open_time_str: new_candle.time.format("%Y-%m-%d %H:%M").to_string(),
            signal,
            volume: risk_dec.volume,
        });
    }

    // --- TIMEOUT: close any trade still open at end of data ---
    if let Some(trade) = open_trade.take() {
        let exit = candles.last().unwrap().close;
        let pnl  = match trade.signal.side {
            Side::Long  => (exit - trade.signal.entry) * trade.volume * contract_size,
            Side::Short => (trade.signal.entry - exit) * trade.volume * contract_size,
        };
        balance += pnl;
        if balance > peak { peak = balance; }
        let dd = balance - peak;
        if dd < max_drawdown { max_drawdown = dd; }
        timeouts  += 1;
        trades    += 1;
        total_pnl += pnl;

        println!(
            "[{} {}] {} {} entry={} tp={} sl={} vol={:.2} → TIMEOUT exit={} pnl={} bal={:.2}",
            trade.open_time_str, tf_short, symbol,
            if trade.signal.side == Side::Long { "LONG " } else { "SHORT" },
            fmt_price(trade.signal.entry, prec),
            fmt_price(trade.signal.tp, prec),
            fmt_price(trade.signal.sl, prec),
            trade.volume,
            fmt_price(exit, prec),
            fmt_pnl(pnl), balance,
        );
    }

    // --- summary ---
    let win_pct     = if trades > 0 { wins     as f64 / trades as f64 * 100.0 } else { 0.0 };
    let loss_pct    = if trades > 0 { losses   as f64 / trades as f64 * 100.0 } else { 0.0 };
    let timeout_pct = if trades > 0 { timeouts as f64 / trades as f64 * 100.0 } else { 0.0 };

    println!("─────────────────────────────────────────");
    println!("Backtest: {} {} | {} candles", symbol, tf_short, total);
    println!("─────────────────────────────────────────");
    println!("Trades     : {}", trades);
    println!("Win        : {}  ({:.1}%)", wins, win_pct);
    println!("Loss       : {}  ({:.1}%)", losses, loss_pct);
    println!("Timeout    : {}  ({:.1}%)", timeouts, timeout_pct);
    println!("─────────────────────────────────────────");
    println!("Total PnL  : {}", fmt_pnl(total_pnl));
    println!("Max Drawdown: {:.2}", max_drawdown);
    println!("Final Balance: {:.2}", balance);
    println!("─────────────────────────────────────────");

    Ok(())
}
