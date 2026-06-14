mod execute;

use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub timeframe:       domain::Timeframe,
    pub candle_count:    u32,
    pub risk_pct:        Decimal,
    pub min_sl_distance: Decimal,
}

#[derive(Debug)]
pub enum EngineOutcome {
    Traded {
        symbol: String,
        volume: Decimal,
        order:  u64,
        side:   domain::Side,
        entry:  Decimal,
        sl:     Decimal,
        tp:     Decimal,
    },
    NoSignal,
    NoApproval,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("mt5 error: {0}")]
    Mt5(#[from] mt5_client::Mt5Error),

    #[error("order rejected: retcode={retcode}, comment={comment}")]
    OrderRejected { retcode: u32, comment: String },
}

struct Candidate {
    symbol: String,
    signal: ict::TradeSignal,
    volume: Decimal,
}

fn risk_config_from_symbol(
    symbol: &domain::Symbol,
    risk_pct: Decimal,
) -> risk::RiskConfig {
    risk::RiskConfig {
        risk_pct,
        value_per_lot: symbol.trade_contract_size,
        min_volume:    symbol.volume_min,
        max_volume:    symbol.volume_max,
        volume_step:   symbol.volume_step,
    }
}

pub async fn run_once(
    symbols: &[&str],
    mt5:     &mt5_client::Mt5Client,
    config:  &EngineConfig,
) -> Result<EngineOutcome, EngineError> {
    let (account, _positions) = tokio::try_join!(mt5.account(), mt5.positions())?;

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut had_signals = false;

    for &sym in symbols {
        let (symbol_info, candles) = match tokio::try_join!(
            mt5.symbol(sym),
            mt5.rates_from_pos(sym, config.timeframe, 0, config.candle_count),
        ) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(symbol = sym, error = %e, "fetch failed, skipping");
                continue;
            }
        };

        let analysis = ict::IctAnalyzer::new(&candles, config.min_sl_distance).analyze();
        let signal = match &analysis.signal {
            Some(s) => s.clone(),
            None    => continue,
        };
        had_signals = true;

        let risk_cfg = risk_config_from_symbol(&symbol_info, config.risk_pct);
        let risk_dec = risk::evaluate(&account, &signal, &risk_cfg);
        if !risk_dec.approved {
            tracing::debug!(symbol = sym, reason = ?risk_dec.reason, "risk rejected");
            continue;
        }

        candidates.push(Candidate { symbol: sym.to_string(), signal, volume: risk_dec.volume });
    }

    let winner = match candidates.into_iter().next() {
        Some(w) => w,
        None => return Ok(if had_signals { EngineOutcome::NoApproval } else { EngineOutcome::NoSignal }),
    };

    let request = execute::build_trade_request(&winner.symbol, &winner.signal, winner.volume);

    match mt5.order_check(&request).await {
        Ok(chk) => tracing::debug!(retcode = chk.retcode, comment = %chk.comment, "order_check"),
        Err(e)  => tracing::warn!(error = %e, "order_check failed, proceeding"),
    }

    let result = mt5.place_order(&request).await?;
    if result.retcode != 10009 {
        return Err(EngineError::OrderRejected { retcode: result.retcode, comment: result.comment });
    }

    tracing::info!(
        symbol = %winner.symbol,
        side   = ?winner.signal.side,
        volume = %winner.volume,
        order  = result.order,
        "trade executed"
    );

    Ok(EngineOutcome::Traded {
        symbol: winner.symbol,
        volume: winner.volume,
        order:  result.order,
        side:   winner.signal.side,
        entry:  winner.signal.entry,
        sl:     winner.signal.sl,
        tp:     winner.signal.tp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_symbol(vol_min: &str, vol_max: &str, vol_step: &str, contract_size: &str) -> domain::Symbol {
        domain::Symbol {
            name:                "TEST".to_string(),
            description:         "Test".to_string(),
            digits:              5,
            point:               "0.00001".parse().unwrap(),
            bid:                 "1.1000".parse().unwrap(),
            ask:                 "1.1001".parse().unwrap(),
            spread:              10,
            spread_float:        true,
            volume_min:          vol_min.parse().unwrap(),
            volume_max:          vol_max.parse().unwrap(),
            volume_step:         vol_step.parse().unwrap(),
            trade_contract_size: contract_size.parse().unwrap(),
            currency_base:       "EUR".to_string(),
            currency_profit:     "USD".to_string(),
            category:            "Forex".to_string(),
        }
    }

    #[test]
    fn risk_config_from_symbol_maps_fields() {
        let sym = make_symbol("0.01", "200.0", "0.01", "100000.0");
        let cfg = risk_config_from_symbol(&sym, "0.01".parse().unwrap());
        assert_eq!(cfg.risk_pct,      "0.01".parse::<Decimal>().unwrap());
        assert_eq!(cfg.value_per_lot, "100000.0".parse::<Decimal>().unwrap());
        assert_eq!(cfg.min_volume,    "0.01".parse::<Decimal>().unwrap());
        assert_eq!(cfg.max_volume,    "200.0".parse::<Decimal>().unwrap());
        assert_eq!(cfg.volume_step,   "0.01".parse::<Decimal>().unwrap());
    }
}
