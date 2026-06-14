use anyhow::Context;
use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};
use domain::Candle;
use domain::Side;
use ict::IctAnalyzer;
use rust_decimal::Decimal;

struct SimTrade {
    open_time_str:    String,
    signal:           ict::TradeSignal,
    volume:           Decimal,
    actual_entry:     Decimal,    // entry price after spread (Long pays ask = entry + spread)
    current_sl:       Decimal,    // may be moved to signal.entry (breakeven) after 1:1
    remaining_volume: Decimal,    // halved after partial TP at 1:1
    partial_pnl:      Decimal,    // P&L banked from partial closes; added to balance at final close
    half_closed:      bool,       // true once 1:1 partial close and/or SL move has fired
    two_r_hit:        bool,       // true once price reached 2R (trailing SL + optional 2R partial)
    open_candle_idx:  usize,      // walk-forward index at open; used by TIMEOUT_CANDLES
}

// --- Friction helpers (unit-testable) ---

/// Actual fill price on entry.
///
/// OHLC candles from MT5 are bid-based.
/// Long: buy at ASK = bid_mid + spread (pay spread upfront).
/// Short: sell at BID = no adjustment needed.
fn actual_entry_price(side: Side, signal_entry: Decimal, spread_price: Decimal) -> Decimal {
    match side {
        Side::Long  => signal_entry + spread_price,
        Side::Short => signal_entry,
    }
}

/// Actual fill price on exit (TP or SL).
///
/// Long TP:  sell at BID = exact TP level (OHLC is bid-based).
/// Long SL:  market sell, BID has dropped past SL, fill = SL - slippage.
/// Short TP: buy to close at ASK = TP_bid + spread.
/// Short SL: market buy, ASK = BID + spread, fill = SL + spread + slippage.
fn actual_exit_price(
    side: Side,
    exit_level: Decimal,
    is_sl: bool,
    spread_price: Decimal,
    slippage_price: Decimal,
) -> Decimal {
    match (side, is_sl) {
        (Side::Long,  false) => exit_level,
        (Side::Long,  true)  => exit_level - slippage_price,
        (Side::Short, false) => exit_level + spread_price,
        (Side::Short, true)  => exit_level + spread_price + slippage_price,
    }
}

// --- Killzone time filter ---

/// A single killzone window as [start_min, end_min) in minutes from midnight.
/// Times are in broker server-time (the same timezone the MT5 bridge reports candles in).
#[derive(Debug, Clone, PartialEq, Eq)]
struct KillzoneWindow {
    start_min: u32,
    end_min:   u32,
}

/// Parse KILLZONE_WINDOWS="HH:MM-HH:MM,HH:MM-HH:MM" into windows.
/// Empty string → no filter (every candle passes).
fn parse_killzone_windows(s: &str) -> anyhow::Result<Vec<KillzoneWindow>> {
    if s.trim().is_empty() {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    for token in s.split(',') {
        let token = token.trim();
        if token.is_empty() { continue; }
        let (a, b) = token.split_once('-')
            .ok_or_else(|| anyhow::anyhow!("invalid killzone window '{token}': expected HH:MM-HH:MM"))?;
        let start_min = parse_hhmm(a.trim())?;
        let end_min   = parse_hhmm(b.trim())?;
        if start_min >= end_min {
            anyhow::bail!("killzone window '{token}': start must be before end");
        }
        result.push(KillzoneWindow { start_min, end_min });
    }
    Ok(result)
}

fn parse_hhmm(s: &str) -> anyhow::Result<u32> {
    let (hh, mm) = s.split_once(':')
        .ok_or_else(|| anyhow::anyhow!("expected HH:MM, got '{s}'"))?;
    let h: u32 = hh.parse().map_err(|_| anyhow::anyhow!("bad hour in '{s}'"))?;
    let m: u32 = mm.parse().map_err(|_| anyhow::anyhow!("bad minute in '{s}'"))?;
    if h > 23 { anyhow::bail!("hour > 23 in '{s}'"); }
    if m > 59 { anyhow::bail!("minute > 59 in '{s}'"); }
    Ok(h * 60 + m)
}

/// Returns true if `t` falls inside any configured killzone window.
/// If no windows are configured, returns true (no filter = every candle passes).
///
/// NOTE: `t` is stored in DateTime<Utc> but holds broker server-time values because
/// the MT5 bridge serialises naive datetimes without a timezone offset.
/// Configure KILLZONE_WINDOWS in broker server-time, not UTC.
fn in_killzone(t: &DateTime<Utc>, windows: &[KillzoneWindow]) -> bool {
    if windows.is_empty() {
        return true;
    }
    let minutes = t.hour() * 60 + t.minute();
    windows.iter().any(|w| minutes >= w.start_min && minutes < w.end_min)
}

/// Average True Range over the last `period` candles in the slice.
fn compute_atr(candles: &[Candle], period: usize) -> Decimal {
    if candles.len() < 2 { return Decimal::ZERO; }
    let n = period.min(candles.len() - 1);
    let start = candles.len() - 1 - n;
    let sum: Decimal = candles[start..candles.len()-1].iter().zip(&candles[start+1..])
        .map(|(prev, cur)| {
            let hl = cur.high - cur.low;
            let hc = (cur.high - prev.close).abs();
            let lc = (cur.low  - prev.close).abs();
            hl.max(hc).max(lc)
        })
        .sum();
    if n == 0 { Decimal::ZERO } else { sum / Decimal::from(n) }
}

/// Rolling EMA for each position in `prices`; returns None until enough data.
fn rolling_ema(prices: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    let k = Decimal::from(2u32) / Decimal::from((period + 1) as u32);
    let mut out = vec![None; prices.len()];
    if prices.len() < period { return out; }
    let seed: Decimal = prices[..period].iter().sum::<Decimal>() / Decimal::from(period);
    out[period - 1] = Some(seed);
    let mut ema = seed;
    for i in period..prices.len() {
        ema = prices[i] * k + ema * (Decimal::ONE - k);
        out[i] = Some(ema);
    }
    out
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

    let mt5_base_url = std::env::var("MT5_BASE_URL").context("MT5_BASE_URL missing")?;
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
    if min_sl_distance < Decimal::ZERO {
        anyhow::bail!("MIN_SL_DISTANCE must be >= 0, got {min_sl_distance}");
    }
    // Extra slippage on SL exits, in symbol points (MT5 minimum price increments).
    // Default = 5 points.  For a 5-decimal forex pair (point=0.00001) this is 0.5 pip.
    let slippage_points = std::env::var("SLIPPAGE_POINTS")
        .unwrap_or_else(|_| "5".to_string())
        .parse::<Decimal>()
        .context("SLIPPAGE_POINTS must be a decimal (e.g. 5)")?;
    if slippage_points < Decimal::ZERO {
        anyhow::bail!("SLIPPAGE_POINTS must be >= 0");
    }
    // Override spread in price units (e.g. SPREAD_OVERRIDE=0 for Zero/Raw accounts).
    // When not set, spread is taken from symbol info as normal.
    let spread_override: Option<Decimal> = match std::env::var("SPREAD_OVERRIDE") {
        Ok(s) => Some(s.parse::<Decimal>().context("SPREAD_OVERRIDE must be a decimal (e.g. 0 or 0.00002)")?),
        Err(_) => None,
    };
    // Round-trip commission per lot in account currency (e.g. 7.0 for Exness Zero $3.5/lot/side).
    // Default = 0.
    let commission_per_lot = std::env::var("COMMISSION_PER_LOT")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<Decimal>()
        .context("COMMISSION_PER_LOT must be a decimal (e.g. 7.0)")?;
    // Swing detection period (n): a candle is a swing high/low if it beats all n
    // candles on both sides. Default = 2. Higher = fewer but more significant swings.
    let swing_period = std::env::var("SWING_PERIOD")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<usize>()
        .context("SWING_PERIOD must be a positive integer")?;
    if swing_period == 0 {
        anyhow::bail!("SWING_PERIOD must be >= 1");
    }
    // Minimum reward-to-risk ratio filter. Signals where (tp-entry)/(entry-sl) < min_rr
    // are discarded. Default = 0 (no filter).
    let min_rr = std::env::var("MIN_RR")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<Decimal>()
        .context("MIN_RR must be a decimal (e.g. 2.0)")?;
    if min_rr < Decimal::ZERO {
        anyhow::bail!("MIN_RR must be >= 0");
    }
    // Killzone windows: comma-separated HH:MM-HH:MM in broker server-time.
    // Empty = no filter (all hours).
    // Example for Exness EET server (UTC+3 summer):
    //   London Open KZ (07:00-10:00 UTC) = "10:00-13:00"
    //   NY Open KZ     (12:00-15:00 UTC) = "15:00-18:00"
    //   Combined: KILLZONE_WINDOWS="10:00-13:00,15:00-18:00"
    let killzone_windows = parse_killzone_windows(
        &std::env::var("KILLZONE_WINDOWS").unwrap_or_default()
    ).context("KILLZONE_WINDOWS parse error")?;

    // ATR-based minimum SL distance: min_sl = ATR(14) * multiplier.
    // Signals whose SL is tighter than this are rejected.  Default = 0 (disabled).
    let atr_sl_mult: Option<Decimal> = match std::env::var("ATR_SL_MULT") {
        Ok(s) => {
            let v = s.parse::<Decimal>().context("ATR_SL_MULT must be a decimal (e.g. 1.0)")?;
            if v > Decimal::ZERO { Some(v) } else { None }
        }
        Err(_) => None,
    };

    // Breakeven SL: after price reaches 1:1 R:R, move SL to signal.entry (breakeven).
    let breakeven_sl_1r = std::env::var("BREAKEVEN_SL_1R")
        .unwrap_or_default().trim().to_lowercase() == "true";
    // Partial TP: close 50% of position when price reaches 1:1 R:R.
    let partial_tp_1r = std::env::var("PARTIAL_TP_1R")
        .unwrap_or_default().trim().to_lowercase() == "true";
    // Time-based exit: close trade after N candles if still open. 0 = disabled.
    let timeout_candles: usize = std::env::var("TIMEOUT_CANDLES")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<usize>()
        .context("TIMEOUT_CANDLES must be a non-negative integer")?;

    // EMA trend filter: only take signals aligned with EMA trend.
    // TREND_EMA_PERIOD=20 → EMA(20).  0 = disabled.
    // TREND_TF=H1|H4 (default H4).  H1 uses existing candles; H4 fetches separately.
    let trend_ema_period: usize = std::env::var("TREND_EMA_PERIOD")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<usize>()
        .context("TREND_EMA_PERIOD must be a non-negative integer")?;
    let trend_tf_h1 = std::env::var("TREND_TF")
        .unwrap_or_else(|_| "H4".to_string())
        .trim()
        .to_uppercase() == "H1";

    // H4 confirmation: require H4 EMA to agree with H1 EMA direction.
    let trend_h4_confirm = std::env::var("TREND_H4_CONFIRM")
        .unwrap_or_default().trim().to_lowercase() == "true";
    let trend_h4_period: usize = std::env::var("TREND_H4_PERIOD")
        .unwrap_or_else(|_| "20".to_string())
        .parse::<usize>()
        .context("TREND_H4_PERIOD must be a positive integer")?;

    // Day-of-week filter: skip Monday (1) and Friday (5).
    let dow_filter = std::env::var("DOW_FILTER")
        .unwrap_or_default().trim().to_lowercase() == "true";

    // Trailing stop: after price hits 2R, move SL to 1R level (locks in 1R profit).
    let trailing_2r = std::env::var("TRAILING_2R")
        .unwrap_or_default().trim().to_lowercase() == "true";

    // Second partial TP: close 25% of original volume when price reaches 2R.
    let partial_tp_2r = std::env::var("PARTIAL_TP_2R")
        .unwrap_or_default().trim().to_lowercase() == "true";

    // Friday session close: force-close any open trade on Friday at or after this hour.
    let friday_close_hour: Option<u32> = match std::env::var("FRIDAY_CLOSE_HOUR") {
        Ok(s) => Some(s.parse::<u32>().context("FRIDAY_CLOSE_HOUR must be 0-23")?),
        Err(_) => None,
    };

    // Minimum ATR(14) in price units. Skip entry if market is too quiet.
    let atr_min_price: Option<Decimal> = match std::env::var("ATR_MIN_PRICE") {
        Ok(s) => {
            let v = s.parse::<Decimal>().context("ATR_MIN_PRICE must be a decimal")?;
            if v > Decimal::ZERO { Some(v) } else { None }
        }
        Err(_) => None,
    };

    // Optional date range filter: YYYY-MM-DD.  Applied to new_candle.time.
    let date_from: Option<NaiveDate> = match std::env::var("DATE_FROM") {
        Ok(s) => Some(s.parse::<NaiveDate>().context("DATE_FROM must be YYYY-MM-DD")?),
        Err(_) => None,
    };
    let date_to: Option<NaiveDate> = match std::env::var("DATE_TO") {
        Ok(s) => Some(s.parse::<NaiveDate>().context("DATE_TO must be YYYY-MM-DD")?),
        Err(_) => None,
    };

    let timeframe = timeframe_str
        .parse::<domain::Timeframe>()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let tf_short = timeframe_str.as_str();

    let mt5 = mt5_client::Mt5Client::new(mt5_base_url);

    tracing::info!(symbol = %symbol, timeframe = ?timeframe, backtest_candles, "fetching historical data");

    let (symbol_info, candles) = tokio::try_join!(
        mt5.symbol(&symbol),
        mt5.rates_from_pos(&symbol, timeframe, 0, backtest_candles),
    )?;

    // Fetch/prepare candles and precompute rolling EMA for trend filter.
    // TREND_TF=H1 → use existing H1 candles (no extra fetch, ema[i] aligns with candles[i]).
    // TREND_TF=H4 → fetch H4 separately; in the loop, binary-search by timestamp.
    let (trend_candles, trend_emas): (Vec<domain::Candle>, Vec<Option<Decimal>>) = if trend_ema_period > 0 {
        if trend_tf_h1 {
            tracing::info!(ema_period = trend_ema_period, tf = "H1", "using H1 EMA trend filter");
            let closes: Vec<Decimal> = candles.iter().map(|c| c.close).collect();
            let emas = rolling_ema(&closes, trend_ema_period);
            (vec![], emas) // empty candles vec signals "use H1 index directly"
        } else {
            tracing::info!(ema_period = trend_ema_period, tf = "H4", "fetching H4 candles for trend filter");
            let h4 = mt5.rates_from_pos(&symbol, domain::Timeframe::H4, 0, backtest_candles / 4 + 200).await?;
            let closes: Vec<Decimal> = h4.iter().map(|c| c.close).collect();
            let emas = rolling_ema(&closes, trend_ema_period);
            (h4, emas)
        }
    } else {
        (vec![], vec![])
    };

    // H4 confirmation filter: fetch H4 EMA independently of the H1 trend filter.
    let (h4c_candles, h4c_emas): (Vec<domain::Candle>, Vec<Option<Decimal>>) = if trend_h4_confirm {
        tracing::info!(ema_period = trend_h4_period, "fetching H4 candles for H4 confirmation");
        let h4 = mt5.rates_from_pos(&symbol, domain::Timeframe::H4, 0, backtest_candles / 4 + 200).await?;
        let closes: Vec<Decimal> = h4.iter().map(|c| c.close).collect();
        let emas = rolling_ema(&closes, trend_h4_period);
        (h4, emas)
    } else {
        (vec![], vec![])
    };

    let total = candles.len();
    if total <= window_size {
        anyhow::bail!("not enough candles: got {total}, need more than {window_size}");
    }

    let contract_size  = symbol_info.trade_contract_size;
    let prec           = symbol_info.digits as usize;
    // spread in price units: use override if set, otherwise read from symbol info
    let spread_price   = spread_override
        .unwrap_or_else(|| Decimal::from(symbol_info.spread) * symbol_info.point);
    let slippage_price = slippage_points * symbol_info.point;

    let profit_is_usd = symbol_info.currency_profit.eq_ignore_ascii_case("USD");
    let risk_cfg_base = risk::RiskConfig {
        risk_pct,
        value_per_lot: contract_size,
        min_volume:    symbol_info.volume_min,
        max_volume:    symbol_info.volume_max,
        volume_step:   symbol_info.volume_step,
    };

    tracing::info!(
        total,
        window_size,
        min_sl_distance  = %min_sl_distance,
        spread_price     = %spread_price,
        slippage_price   = %slippage_price,
        killzone_windows = ?killzone_windows,
        "starting walk-forward"
    );

    let mut balance      = backtest_balance;
    let mut account_sim  = make_sim_account(balance);
    let mut open_trade: Option<SimTrade> = None;
    let mut peak         = balance;
    let mut max_drawdown = Decimal::ZERO;

    let mut trades          = 0u32;
    let mut wins            = 0u32;
    let mut losses          = 0u32;
    let mut timeouts        = 0u32;
    let mut be_sl_exits     = 0u32; // trades stopped at breakeven SL (after 1:1 move)
    let mut skipped_kz      = 0u32; // candles outside killzone windows
    let mut missed_fills    = 0u32; // signals approved but candle never reached entry
    let mut total_pnl       = Decimal::ZERO;
    let mut total_friction  = Decimal::ZERO;
    let mut sum_wins        = Decimal::ZERO;
    let mut sum_losses      = Decimal::ZERO;
    let mut max_consec_loss = 0u32;
    let mut cur_consec_loss = 0u32;

    for i in 0..(total - window_size) {
        let new_idx    = i + window_size;
        let new_candle = &candles[new_idx];

        let candle_date = new_candle.time.date_naive();

        // --- check open trade (always runs — SL/TP management is not date-filtered) ---
        if open_trade.is_some() {
            // 0. Friday session close: force-exit before weekend gap.
            if let Some(close_hour) = friday_close_hour {
                if open_trade.is_some()
                    && new_candle.time.weekday().number_from_monday() == 5
                    && new_candle.time.hour() >= close_hour
                {
                    let trade = open_trade.take().unwrap();
                    let exit_level = new_candle.close;
                    let exit = actual_exit_price(trade.signal.side, exit_level, false, spread_price, slippage_price);
                    let commission = commission_per_lot * trade.remaining_volume;
                    let profit_rate = if profit_is_usd || exit <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit };
                    let fl_rate = if profit_is_usd || exit_level <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit_level };
                    let final_pnl = (match trade.signal.side {
                        Side::Long  => (exit - trade.actual_entry) * trade.remaining_volume * contract_size,
                        Side::Short => (trade.actual_entry - exit) * trade.remaining_volume * contract_size,
                    }) * profit_rate - commission;
                    let frictionless_pnl = (match trade.signal.side {
                        Side::Long  => (exit_level - trade.signal.entry) * trade.remaining_volume * contract_size,
                        Side::Short => (trade.signal.entry - exit_level) * trade.remaining_volume * contract_size,
                    }) * fl_rate;
                    let friction = frictionless_pnl - final_pnl;
                    let pnl = trade.partial_pnl + final_pnl;
                    balance += pnl;
                    if balance > peak { peak = balance; }
                    let dd = balance - peak;
                    if dd < max_drawdown { max_drawdown = dd; }
                    timeouts       += 1;
                    trades         += 1;
                    total_pnl      += pnl;
                    total_friction += friction;
                    account_sim = make_sim_account(balance);
                    println!(
                        "[{} {}] {} {} entry={} tp={} sl={} vol={:.2} → FRI_CLS exit={} friction={} pnl={} bal={:.2}",
                        trade.open_time_str, tf_short, symbol,
                        if trade.signal.side == Side::Long { "LONG " } else { "SHORT" },
                        fmt_price(trade.actual_entry, prec),
                        fmt_price(trade.signal.tp, prec),
                        fmt_price(trade.signal.sl, prec),
                        trade.volume,
                        fmt_price(exit, prec),
                        fmt_pnl(-friction),
                        fmt_pnl(pnl), balance,
                    );
                }
            }

            // 1. Time-based candle timeout: close stale trade at market price.
            if timeout_candles > 0 {
                if let Some(t) = open_trade.as_ref() {
                    if new_idx >= t.open_candle_idx + timeout_candles {
                        let trade = open_trade.take().unwrap();
                        let exit_level = new_candle.close;
                        let exit = actual_exit_price(trade.signal.side, exit_level, false, spread_price, slippage_price);
                        let commission = commission_per_lot * trade.remaining_volume;
                        let profit_rate = if profit_is_usd || exit <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit };
                        let fl_rate = if profit_is_usd || exit_level <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit_level };
                        let final_pnl = (match trade.signal.side {
                            Side::Long  => (exit - trade.actual_entry) * trade.remaining_volume * contract_size,
                            Side::Short => (trade.actual_entry - exit) * trade.remaining_volume * contract_size,
                        }) * profit_rate - commission;
                        let frictionless_pnl = (match trade.signal.side {
                            Side::Long  => (exit_level - trade.signal.entry) * trade.remaining_volume * contract_size,
                            Side::Short => (trade.signal.entry - exit_level) * trade.remaining_volume * contract_size,
                        }) * fl_rate;
                        let friction = frictionless_pnl - final_pnl;
                        let pnl = trade.partial_pnl + final_pnl;
                        balance += pnl;
                        if balance > peak { peak = balance; }
                        let dd = balance - peak;
                        if dd < max_drawdown { max_drawdown = dd; }
                        timeouts       += 1;
                        trades         += 1;
                        total_pnl      += pnl;
                        total_friction += friction;
                        account_sim = make_sim_account(balance);
                        println!(
                            "[{} {}] {} {} entry={} tp={} sl={} vol={:.2} → TIMEOUT exit={} friction={} pnl={} bal={:.2}",
                            trade.open_time_str, tf_short, symbol,
                            if trade.signal.side == Side::Long { "LONG " } else { "SHORT" },
                            fmt_price(trade.actual_entry, prec),
                            fmt_price(trade.signal.tp, prec),
                            fmt_price(trade.signal.sl, prec),
                            trade.volume,
                            fmt_price(exit, prec),
                            fmt_pnl(-friction),
                            fmt_pnl(pnl), balance,
                        );
                    }
                }
            }

            // 2. 1:1 events: partial TP and/or breakeven SL (fires once per trade).
            if breakeven_sl_1r || partial_tp_1r {
                if let Some(t) = open_trade.as_ref() {
                    if !t.half_closed {
                        let one_r_level = t.signal.entry + (t.signal.entry - t.signal.sl);
                        let one_r_hit = match t.signal.side {
                            Side::Long  => new_candle.high >= one_r_level,
                            Side::Short => new_candle.low  <= one_r_level,
                        };
                        if one_r_hit {
                            let t = open_trade.as_mut().unwrap();
                            if partial_tp_1r {
                                let partial_vol = (t.volume / Decimal::from(2u32) / risk_cfg_base.volume_step).floor()
                                    * risk_cfg_base.volume_step;
                                if partial_vol >= risk_cfg_base.min_volume && t.remaining_volume > partial_vol {
                                    let exit = actual_exit_price(t.signal.side, one_r_level, false, spread_price, slippage_price);
                                    let pr = if profit_is_usd || exit <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit };
                                    let pp = (match t.signal.side {
                                        Side::Long  => (exit - t.actual_entry) * partial_vol * contract_size,
                                        Side::Short => (t.actual_entry - exit) * partial_vol * contract_size,
                                    }) * pr - commission_per_lot * partial_vol;
                                    t.partial_pnl      += pp;
                                    t.remaining_volume -= partial_vol;
                                    println!(
                                        "[{} {}] {} {} entry={} → PARTIAL1R exit={} vol={:.2} pnl={} (rem={:.2})",
                                        t.open_time_str, tf_short, symbol,
                                        if t.signal.side == Side::Long { "LONG " } else { "SHORT" },
                                        fmt_price(t.actual_entry, prec),
                                        fmt_price(exit, prec),
                                        partial_vol,
                                        fmt_pnl(pp),
                                        t.remaining_volume,
                                    );
                                }
                            }
                            if breakeven_sl_1r {
                                t.current_sl = t.signal.entry;
                            }
                            t.half_closed = true;
                        }
                    }
                }
            }

            // 3. 2R events: trailing SL to 1R and/or second partial TP (fires once per trade).
            if trailing_2r || partial_tp_2r {
                if let Some(t) = open_trade.as_ref() {
                    if !t.two_r_hit {
                        let one_r_dist  = (t.signal.entry - t.signal.sl).abs();
                        let two_r_level = match t.signal.side {
                            Side::Long  => t.signal.entry + one_r_dist * Decimal::from(2u32),
                            Side::Short => t.signal.entry - one_r_dist * Decimal::from(2u32),
                        };
                        let two_r_reached = match t.signal.side {
                            Side::Long  => new_candle.high >= two_r_level,
                            Side::Short => new_candle.low  <= two_r_level,
                        };
                        if two_r_reached {
                            let t = open_trade.as_mut().unwrap();
                            if partial_tp_2r {
                                let partial_vol = (t.volume / Decimal::from(4u32) / risk_cfg_base.volume_step).floor()
                                    * risk_cfg_base.volume_step;
                                if partial_vol >= risk_cfg_base.min_volume && t.remaining_volume > partial_vol {
                                    let exit = actual_exit_price(t.signal.side, two_r_level, false, spread_price, slippage_price);
                                    let pr = if profit_is_usd || exit <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit };
                                    let pp = (match t.signal.side {
                                        Side::Long  => (exit - t.actual_entry) * partial_vol * contract_size,
                                        Side::Short => (t.actual_entry - exit) * partial_vol * contract_size,
                                    }) * pr - commission_per_lot * partial_vol;
                                    t.partial_pnl      += pp;
                                    t.remaining_volume -= partial_vol;
                                    println!(
                                        "[{} {}] {} {} entry={} → PARTIAL2R exit={} vol={:.2} pnl={} (rem={:.2})",
                                        t.open_time_str, tf_short, symbol,
                                        if t.signal.side == Side::Long { "LONG " } else { "SHORT" },
                                        fmt_price(t.actual_entry, prec),
                                        fmt_price(exit, prec),
                                        partial_vol,
                                        fmt_pnl(pp),
                                        t.remaining_volume,
                                    );
                                }
                            }
                            if trailing_2r {
                                let one_r_level = match t.signal.side {
                                    Side::Long  => t.signal.entry + one_r_dist,
                                    Side::Short => t.signal.entry - one_r_dist,
                                };
                                let improves = match t.signal.side {
                                    Side::Long  => one_r_level > t.current_sl,
                                    Side::Short => one_r_level < t.current_sl,
                                };
                                if improves { t.current_sl = one_r_level; }
                            }
                            t.two_r_hit = true;
                        }
                    }
                }
            }

            // 4. SL/TP check — uses current_sl which may have been moved to breakeven.
            if let Some(t) = open_trade.as_ref() {
                let (sl_hit, tp_hit) = match t.signal.side {
                    Side::Long  => (
                        new_candle.low  <= t.current_sl,
                        new_candle.high >= t.signal.tp,
                    ),
                    Side::Short => (
                        new_candle.high >= t.current_sl,
                        new_candle.low  <= t.signal.tp,
                    ),
                };

                if sl_hit || tp_hit {
                    let trade  = open_trade.take().unwrap();
                    let is_sl  = sl_hit; // SL takes priority when both hit on same candle
                    let is_be  = is_sl && trade.current_sl != trade.signal.sl;
                    let exit_level = if is_sl { trade.current_sl } else { trade.signal.tp };
                    let label  = if tp_hit { "TP     " } else if is_be { "BE_SL  " } else { "SL     " };

                    let exit = actual_exit_price(
                        trade.signal.side,
                        exit_level,
                        is_sl,
                        spread_price,
                        slippage_price,
                    );

                    let commission = commission_per_lot * trade.remaining_volume;
                    let profit_rate = if profit_is_usd || exit <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit };
                    let final_pnl = (match trade.signal.side {
                        Side::Long  => (exit - trade.actual_entry) * trade.remaining_volume * contract_size,
                        Side::Short => (trade.actual_entry - exit) * trade.remaining_volume * contract_size,
                    }) * profit_rate - commission;

                    let fl_rate = if profit_is_usd || exit_level <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit_level };
                    let frictionless_pnl = (match trade.signal.side {
                        Side::Long  => (exit_level - trade.signal.entry) * trade.remaining_volume * contract_size,
                        Side::Short => (trade.signal.entry - exit_level) * trade.remaining_volume * contract_size,
                    }) * fl_rate;
                    let friction = frictionless_pnl - final_pnl;

                    // Total P&L = partial close (at 1:1) + final close (at SL/TP).
                    let pnl = trade.partial_pnl + final_pnl;
                    balance += pnl;
                    if balance > peak { peak = balance; }
                    let dd = balance - peak;
                    if dd < max_drawdown { max_drawdown = dd; }

                    if is_be { be_sl_exits += 1; }

                    // Classify win/loss by total trade P&L (partial + final combined).
                    if pnl >= Decimal::ZERO {
                        wins            += 1;
                        sum_wins        += pnl;
                        cur_consec_loss  = 0;
                    } else {
                        losses          += 1;
                        sum_losses      += pnl.abs();
                        cur_consec_loss += 1;
                        if cur_consec_loss > max_consec_loss { max_consec_loss = cur_consec_loss; }
                    }
                    trades          += 1;
                    total_pnl       += pnl;
                    total_friction  += friction;
                    account_sim = make_sim_account(balance);

                    println!(
                        "[{} {}] {} {} entry={} tp={} sl={} vol={:.2} → {} exit={} friction={} pnl={} bal={:.2}",
                        trade.open_time_str, tf_short, symbol,
                        if trade.signal.side == Side::Long { "LONG " } else { "SHORT" },
                        fmt_price(trade.actual_entry, prec),
                        fmt_price(trade.signal.tp, prec),
                        fmt_price(trade.signal.sl, prec),
                        trade.volume, label,
                        fmt_price(exit, prec),
                        fmt_pnl(-friction),
                        fmt_pnl(pnl), balance,
                    );
                }
            }
            continue; // trade was open (or just closed) — skip new signal on same candle
        }

        // --- no open trade: run pipeline ---

        // Date range filter: only enter new trades within the specified range.
        if date_from.is_some_and(|d| candle_date < d) { continue; }
        if date_to.is_some_and(|d| candle_date > d)   { continue; }

        // Killzone filter: only look for entries during configured sessions.
        if !in_killzone(&new_candle.time, &killzone_windows) {
            skipped_kz += 1;
            continue;
        }

        // Day-of-week filter: skip Monday (1) and Friday (5).
        if dow_filter {
            let wd = new_candle.time.weekday().number_from_monday();
            if wd == 1 || wd == 5 { continue; }
        }

        let window = &candles[i..new_idx];

        // ATR-based minimum SL: compute dynamically from window, or fall back to fixed.
        let current_atr = compute_atr(window, 14);
        let effective_min_sl = atr_sl_mult
            .map(|m| current_atr * m)
            .unwrap_or(min_sl_distance);

        // ATR minimum volatility: skip if market is too quiet.
        if let Some(min_atr) = atr_min_price {
            if current_atr < min_atr { continue; }
        }

        let analysis = IctAnalyzer::new(window, effective_min_sl).with_swing_period(swing_period).analyze();
        let signal   = match analysis.signal.as_ref() {
            Some(s) => s.clone(),
            None    => continue,
        };

        // EMA trend filter: skip signals that oppose the EMA trend.
        if trend_ema_period > 0 {
            let (trend_close, ema_val) = if trend_tf_h1 {
                // H1: new_idx is the index into candles[] for new_candle
                let ema = trend_emas.get(new_idx).copied().flatten();
                (new_candle.close, ema)
            } else {
                // H4: binary-search for latest H4 candle <= new_candle.time
                let idx = trend_candles.partition_point(|c| c.time <= new_candle.time);
                if idx == 0 { continue; }
                let i = idx - 1;
                (trend_candles[i].close, trend_emas.get(i).copied().flatten())
            };
            if let Some(ema) = ema_val {
                let trend_up = trend_close > ema;
                match signal.side {
                    Side::Long  if !trend_up => continue,
                    Side::Short if  trend_up => continue,
                    _ => {}
                }
            }
        }

        // H4 confirmation: both primary EMA (H1) and H4 EMA must agree with signal direction.
        if trend_h4_confirm && !h4c_candles.is_empty() {
            let idx = h4c_candles.partition_point(|c| c.time <= new_candle.time);
            if idx == 0 { continue; }
            let hi = idx - 1;
            if let Some(ema) = h4c_emas.get(hi).copied().flatten() {
                let h4_up = h4c_candles[hi].close > ema;
                match signal.side {
                    Side::Long  if !h4_up => continue,
                    Side::Short if  h4_up => continue,
                    _ => {}
                }
            }
        }

        // Minimum R:R filter.
        if min_rr > Decimal::ZERO {
            let reward = (signal.tp - signal.entry).abs();
            let risk   = (signal.entry - signal.sl).abs();
            if risk == Decimal::ZERO || reward / risk < min_rr {
                continue;
            }
        }

        // For pairs where profit is not in USD (JPY, CHF, CAD), convert value_per_lot
        // to account currency using the current candle close as the exchange rate.
        let risk_cfg = if profit_is_usd {
            risk_cfg_base.clone()
        } else {
            let mut c = risk_cfg_base.clone();
            if new_candle.close > Decimal::ZERO {
                c.value_per_lot = contract_size / new_candle.close;
            }
            c
        };
        let risk_dec = risk::evaluate(&account_sim, &signal, &risk_cfg);
        if !risk_dec.approved {
            continue;
        }

        // Fill validation: limit-order semantics.
        // OHLC is bid-based. Long limit fills only when bid drops to entry level;
        // Short limit fills only when bid rises to entry level.
        let fill_ok = match signal.side {
            Side::Long  => new_candle.low  <= signal.entry,
            Side::Short => new_candle.high >= signal.entry,
        };
        if !fill_ok {
            missed_fills += 1;
            tracing::debug!(
                side = ?signal.side,
                entry = %signal.entry,
                candle_low  = %new_candle.low,
                candle_high = %new_candle.high,
                "signal entry not reached — limit order unfilled"
            );
            continue;
        }

        let ae = actual_entry_price(signal.side, signal.entry, spread_price);
        let initial_sl = signal.sl;
        let initial_vol = risk_dec.volume;
        open_trade = Some(SimTrade {
            open_time_str:    new_candle.time.format("%Y-%m-%d %H:%M").to_string(),
            signal,
            volume:           initial_vol,
            actual_entry:     ae,
            current_sl:       initial_sl,
            remaining_volume: initial_vol,
            partial_pnl:      Decimal::ZERO,
            half_closed:      false,
            two_r_hit:        false,
            open_candle_idx:  new_idx,
        });
    }

    // --- END-OF-DATA TIMEOUT: close any trade still open at end of data ---
    if let Some(trade) = open_trade.take() {
        let exit_level = candles.last().unwrap().close;
        let exit = actual_exit_price(trade.signal.side, exit_level, false, spread_price, slippage_price);
        let commission = commission_per_lot * trade.remaining_volume;
        let profit_rate = if profit_is_usd || exit <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit };
        let fl_rate = if profit_is_usd || exit_level <= Decimal::ZERO { Decimal::ONE } else { Decimal::ONE / exit_level };
        let final_pnl = (match trade.signal.side {
            Side::Long  => (exit - trade.actual_entry) * trade.remaining_volume * contract_size,
            Side::Short => (trade.actual_entry - exit) * trade.remaining_volume * contract_size,
        }) * profit_rate - commission;
        let frictionless_pnl = (match trade.signal.side {
            Side::Long  => (exit_level - trade.signal.entry) * trade.remaining_volume * contract_size,
            Side::Short => (trade.signal.entry - exit_level) * trade.remaining_volume * contract_size,
        }) * fl_rate;
        let friction = frictionless_pnl - final_pnl;
        let pnl = trade.partial_pnl + final_pnl;

        balance += pnl;
        if balance > peak { peak = balance; }
        let dd = balance - peak;
        if dd < max_drawdown { max_drawdown = dd; }
        timeouts        += 1;
        trades          += 1;
        total_pnl       += pnl;
        total_friction  += friction;

        println!(
            "[{} {}] {} {} entry={} tp={} sl={} vol={:.2} → TIMEOUT exit={} friction={} pnl={} bal={:.2}",
            trade.open_time_str, tf_short, symbol,
            if trade.signal.side == Side::Long { "LONG " } else { "SHORT" },
            fmt_price(trade.actual_entry, prec),
            fmt_price(trade.signal.tp, prec),
            fmt_price(trade.signal.sl, prec),
            trade.volume,
            fmt_price(exit, prec),
            fmt_pnl(-friction),
            fmt_pnl(pnl), balance,
        );
    }

    // --- derived metrics ---
    let win_pct        = if trades > 0 { wins     as f64 / trades as f64 * 100.0 } else { 0.0 };
    let loss_pct       = if trades > 0 { losses   as f64 / trades as f64 * 100.0 } else { 0.0 };
    let timeout_pct    = if trades > 0 { timeouts as f64 / trades as f64 * 100.0 } else { 0.0 };
    let avg_win        = if wins   > 0 { sum_wins   / Decimal::from(wins)   } else { Decimal::ZERO };
    let avg_loss       = if losses > 0 { sum_losses / Decimal::from(losses) } else { Decimal::ZERO };
    let expectancy     = if trades > 0 {
        total_pnl / Decimal::from(trades)
    } else {
        Decimal::ZERO
    };
    let profit_factor  = if sum_losses > Decimal::ZERO {
        sum_wins / sum_losses
    } else {
        Decimal::MAX
    };
    let return_pct     = (balance - backtest_balance) / backtest_balance * Decimal::from(100u32);

    println!("─────────────────────────────────────────");
    println!("Backtest: {} {} | {} candles", symbol, tf_short, total);
    println!("Friction: spread={} slippage={} commission/lot={} (per trade avg={})",
        fmt_price(spread_price, prec),
        fmt_price(slippage_price, prec),
        fmt_price(commission_per_lot, 2),
        fmt_price(if trades > 0 { total_friction / Decimal::from(trades) } else { Decimal::ZERO }, 2),
    );
    println!("─────────────────────────────────────────");
    let kz_label = if killzone_windows.is_empty() {
        "all hours (no filter)".to_string()
    } else {
        killzone_windows.iter()
            .map(|w| format!("{:02}:{:02}-{:02}:{:02}", w.start_min/60, w.start_min%60, w.end_min/60, w.end_min%60))
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!("Killzone filter: {}", kz_label);
    println!("Skipped (KZ)   : {}", skipped_kz);
    println!("─────────────────────────────────────────");
    println!("Trades         : {}", trades);
    println!("Win            : {}  ({:.1}%)", wins, win_pct);
    println!("Loss           : {}  ({:.1}%)", losses, loss_pct);
    println!("Timeout        : {}  ({:.1}%)", timeouts, timeout_pct);
    if be_sl_exits > 0 || breakeven_sl_1r {
        println!("BE-SL exits    : {}", be_sl_exits);
    }
    println!("Missed fills   : {}", missed_fills);
    println!("Max consec loss: {}", max_consec_loss);
    println!("─────────────────────────────────────────");
    println!("Avg win        : +{:.2}", avg_win);
    println!("Avg loss       : -{:.2}", avg_loss);
    println!("Expectancy     : {}", fmt_pnl(expectancy));
    println!("Profit factor  : {:.2}", profit_factor);
    println!("Total friction : {}", fmt_pnl(-total_friction));
    println!("─────────────────────────────────────────");
    println!("Total PnL      : {}", fmt_pnl(total_pnl));
    println!("Max Drawdown   : {:.2}", max_drawdown);
    println!("Return         : {:.1}%", return_pct);
    println!("Final Balance  : {:.2}", balance);
    println!("─────────────────────────────────────────");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal { s.parse().unwrap() }

    // --- actual_entry_price ---

    #[test]
    fn long_entry_pays_spread() {
        let price = actual_entry_price(Side::Long, d("1.3000"), d("0.0002"));
        assert_eq!(price, d("1.3002"));
    }

    #[test]
    fn short_entry_no_spread_adjustment() {
        // OHLC is bid-based; Short sells at BID = no adjustment
        let price = actual_entry_price(Side::Short, d("1.3000"), d("0.0002"));
        assert_eq!(price, d("1.3000"));
    }

    #[test]
    fn zero_spread_entry_unchanged() {
        let long  = actual_entry_price(Side::Long,  d("1.3000"), Decimal::ZERO);
        let short = actual_entry_price(Side::Short, d("1.3000"), Decimal::ZERO);
        assert_eq!(long,  d("1.3000"));
        assert_eq!(short, d("1.3000"));
    }

    // --- actual_exit_price ---

    #[test]
    fn long_tp_exit_exact() {
        // Long TP: sell at BID = exact TP level
        let price = actual_exit_price(Side::Long, d("1.3100"), false, d("0.0002"), d("0.0001"));
        assert_eq!(price, d("1.3100"));
    }

    #[test]
    fn long_sl_exit_loses_slippage() {
        // Long SL: market sell, get SL - slippage
        let price = actual_exit_price(Side::Long, d("1.2900"), true, d("0.0002"), d("0.0001"));
        assert_eq!(price, d("1.2899"));
    }

    #[test]
    fn short_tp_exit_pays_spread() {
        // Short TP: buy to close at ASK = TP_bid + spread
        let price = actual_exit_price(Side::Short, d("1.2900"), false, d("0.0002"), d("0.0001"));
        assert_eq!(price, d("1.2902"));
    }

    #[test]
    fn short_sl_exit_pays_spread_and_slippage() {
        // Short SL: market buy at ASK + slippage
        let price = actual_exit_price(Side::Short, d("1.3200"), true, d("0.0002"), d("0.0001"));
        assert_eq!(price, d("1.3203"));
    }

    #[test]
    fn zero_friction_exit_matches_level() {
        let long_tp  = actual_exit_price(Side::Long,  d("1.3100"), false, Decimal::ZERO, Decimal::ZERO);
        let long_sl  = actual_exit_price(Side::Long,  d("1.2900"), true,  Decimal::ZERO, Decimal::ZERO);
        let short_tp = actual_exit_price(Side::Short, d("1.2900"), false, Decimal::ZERO, Decimal::ZERO);
        let short_sl = actual_exit_price(Side::Short, d("1.3200"), true,  Decimal::ZERO, Decimal::ZERO);
        assert_eq!(long_tp,  d("1.3100"));
        assert_eq!(long_sl,  d("1.2900"));
        assert_eq!(short_tp, d("1.2900"));
        assert_eq!(short_sl, d("1.3200"));
    }

    // --- killzone filter ---

    fn make_utc(h: u32, m: u32) -> DateTime<Utc> {
        use chrono::TimeZone;
        Utc.with_ymd_and_hms(2026, 1, 1, h, m, 0).unwrap()
    }

    #[test]
    fn empty_windows_passes_all() {
        let windows = parse_killzone_windows("").unwrap();
        assert!(in_killzone(&make_utc(0,  0), &windows));
        assert!(in_killzone(&make_utc(3, 30), &windows));
        assert!(in_killzone(&make_utc(23,59), &windows));
    }

    #[test]
    fn single_window_inside_outside() {
        let windows = parse_killzone_windows("10:00-13:00").unwrap();
        assert!( in_killzone(&make_utc(10, 0),  &windows)); // boundary start
        assert!( in_killzone(&make_utc(11, 30), &windows)); // inside
        assert!(!in_killzone(&make_utc(13, 0),  &windows)); // boundary end (exclusive)
        assert!(!in_killzone(&make_utc(9, 59),  &windows)); // before
        assert!(!in_killzone(&make_utc(14, 0),  &windows)); // after
    }

    #[test]
    fn two_windows_union() {
        let windows = parse_killzone_windows("10:00-13:00,15:00-18:00").unwrap();
        assert!( in_killzone(&make_utc(10, 30), &windows));
        assert!(!in_killzone(&make_utc(13, 30), &windows)); // gap between windows
        assert!( in_killzone(&make_utc(16,  0), &windows));
        assert!(!in_killzone(&make_utc(18,  0), &windows)); // end exclusive
    }

    #[test]
    fn parse_invalid_format_errors() {
        assert!(parse_killzone_windows("bad").is_err());
        assert!(parse_killzone_windows("10:00").is_err()); // no range
        assert!(parse_killzone_windows("13:00-10:00").is_err()); // end <= start
        assert!(parse_killzone_windows("25:00-26:00").is_err()); // hour out of range
    }

    // --- fill_ok logic ---

    fn fill_ok(side: Side, entry: Decimal, candle_low: Decimal, candle_high: Decimal) -> bool {
        match side {
            Side::Long  => candle_low  <= entry,
            Side::Short => candle_high >= entry,
        }
    }

    #[test]
    fn long_fill_when_candle_touches_entry() {
        assert!( fill_ok(Side::Long, d("1.3000"), d("1.2990"), d("1.3050"))); // low<=entry
        assert!(!fill_ok(Side::Long, d("1.3000"), d("1.3010"), d("1.3050"))); // gap — low>entry
    }

    #[test]
    fn short_fill_when_candle_touches_entry() {
        assert!( fill_ok(Side::Short, d("1.3000"), d("1.2950"), d("1.3010"))); // high>=entry
        assert!(!fill_ok(Side::Short, d("1.3000"), d("1.2900"), d("1.2990"))); // gap — high<entry
    }

    #[test]
    fn fill_at_exact_boundary() {
        assert!(fill_ok(Side::Long,  d("1.3000"), d("1.3000"), d("1.3050"))); // low==entry
        assert!(fill_ok(Side::Short, d("1.3000"), d("1.2950"), d("1.3000"))); // high==entry
    }

    // --- PnL direction sanity ---

    #[test]
    fn long_tp_pnl_positive_with_spread() {
        // Long: entry=1.3002 (after spread), tp_exit=1.3100
        let ae   = actual_entry_price(Side::Long, d("1.3000"), d("0.0002"));
        let exit = actual_exit_price(Side::Long, d("1.3100"), false, d("0.0002"), d("0.0001"));
        let pnl  = (exit - ae) * d("0.10") * d("100000");
        assert!(pnl > Decimal::ZERO, "long TP should be profitable: {pnl}");
    }

    #[test]
    fn short_sl_pnl_negative_and_worse_than_frictionless() {
        // Short: entry=1.3000 (bid), sl=1.3100 (above entry = loss), with friction
        let ae           = actual_entry_price(Side::Short, d("1.3000"), d("0.0002"));
        let exit         = actual_exit_price(Side::Short, d("1.3100"), true, d("0.0002"), d("0.0001"));
        let pnl          = (ae - exit) * d("0.10") * d("100000");
        let frictionless = (d("1.3000") - d("1.3100")) * d("0.10") * d("100000");
        assert!(pnl < Decimal::ZERO,        "short SL must be a loss: {pnl}");
        assert!(pnl < frictionless,          "friction must make loss larger: pnl={pnl} frictionless={frictionless}");
    }
}
