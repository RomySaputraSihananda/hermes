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
        action: agents::Action,
        volume: Decimal,
        order:  u64,
    },
    NoSignal,
    Hold,
    NoApproval,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("mt5 error: {0}")]
    Mt5(#[from] mt5_client::Mt5Error),

    #[error("agents error: {0}")]
    Agents(#[from] agents::AgentsError),

    #[error("order rejected: retcode={retcode}, comment={comment}")]
    OrderRejected { retcode: u32, comment: String },
}

#[derive(Clone)]
struct Candidate {
    symbol:   String,
    signal:   ict::TradeSignal,
    decision: agents::AgentDecision,
    volume:   Decimal,
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

fn select_winner(candidates: Vec<Candidate>) -> Option<Candidate> {
    candidates.into_iter().max_by(|a, b| {
        a.decision.confidence
            .partial_cmp(&b.decision.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

pub async fn run_once(
    symbols: &[&str],
    mt5:     &mt5_client::Mt5Client,
    llm:     &openrouter::OpenRouterClient,
    model:   &str,
    config:  &EngineConfig,
) -> Result<EngineOutcome, EngineError> {
    // Tahap 1: fetch account + positions in parallel
    let (account, positions) = tokio::try_join!(mt5.account(), mt5.positions())?;

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut had_signals  = false;
    let mut had_buy_sell = false;

    // Tahap 2: per-symbol fetch + analyze (sequential — clients are not Clone)
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

        let technical_in   = agents::TechnicalInput   { symbol: sym, candles: &candles, analysis: &analysis };
        let sentiment_in   = agents::SentimentInput   { symbol: sym, candles: &candles };
        let fundamental_in = agents::FundamentalInput { symbol: sym, news_context: "" };
        let risk_in        = agents::RiskInput        { account: &account, positions: &positions, signal: Some(&signal) };

        let ict_action = match signal.side {
            domain::Side::Long  => agents::Action::Buy,
            domain::Side::Short => agents::Action::Sell,
        };
        let decision = match agents::run_agents(llm, model, technical_in, sentiment_in, fundamental_in, risk_in).await {
            Ok(d)  => d,
            Err(e) => {
                tracing::warn!(error = %e, "agents unavailable after retry, falling back to ICT signal");
                agents::AgentDecision::passthrough(ict_action)
            }
        };

        if decision.action == agents::Action::Hold {
            continue;
        }
        had_buy_sell = true;

        let risk_cfg = risk_config_from_symbol(&symbol_info, config.risk_pct);
        let risk_dec = risk::evaluate(&account, &signal, &risk_cfg);
        if !risk_dec.approved {
            tracing::debug!(symbol = sym, reason = ?risk_dec.reason, "risk rejected");
            continue;
        }

        candidates.push(Candidate { symbol: sym.to_string(), signal, decision, volume: risk_dec.volume });
    }

    // Tahap 3: pick winner
    let winner = match select_winner(candidates) {
        Some(w) => w,
        None => {
            return Ok(if !had_signals {
                EngineOutcome::NoSignal
            } else if !had_buy_sell {
                EngineOutcome::Hold
            } else {
                EngineOutcome::NoApproval
            });
        }
    };

    // Tahap 4: execute
    let request = execute::build_trade_request(&winner.symbol, &winner.signal, winner.volume);

    // Pre-flight check — log only, do not abort
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
        action = ?winner.decision.action,
        volume = %winner.volume,
        order  = result.order,
        "trade executed"
    );

    Ok(EngineOutcome::Traded {
        symbol: winner.symbol,
        action: winner.decision.action,
        volume: winner.volume,
        order:  result.order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agents::{Action, AgentDecision, AgentVote};
    use ict::{ConfluenceFlags, TradeSignal};

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

    fn make_candidate(symbol: &str, confidence: f64) -> Candidate {
        let vote = AgentVote {
            action:    Action::Buy,
            confidence,
            reasoning: String::new(),
        };
        Candidate {
            symbol: symbol.to_string(),
            signal: TradeSignal {
                side:       domain::Side::Long,
                entry:      "1.1000".parse().unwrap(),
                sl:         "1.0950".parse().unwrap(),
                tp:         "1.1100".parse().unwrap(),
                confluence: ConfluenceFlags::default(),
            },
            decision: AgentDecision {
                action:      Action::Buy,
                confidence,
                reasoning:   String::new(),
                votes:       [vote.clone(), vote.clone(), vote.clone(), vote.clone()],
                from_quorum: true,
            },
            volume: "0.10".parse().unwrap(),
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

    #[test]
    fn select_winner_picks_highest_confidence() {
        let candidates = vec![
            make_candidate("EURUSD", 0.7),
            make_candidate("GBPUSD", 0.9),
            make_candidate("USDJPY", 0.6),
        ];
        let winner = select_winner(candidates).unwrap();
        assert_eq!(winner.symbol, "GBPUSD");
    }

    #[test]
    fn select_winner_empty_returns_none() {
        assert!(select_winner(vec![]).is_none());
    }
}
