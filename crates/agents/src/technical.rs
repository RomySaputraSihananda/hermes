use crate::error::AgentsError;
use crate::types::{AgentVote, agent_vote_schema};
use domain::Candle;
use ict::IctAnalysis;
use openrouter::{Message, OpenRouterClient, Role};

pub struct TechnicalInput<'a> {
    pub symbol: &'a str,
    pub candles: &'a [Candle],
    pub analysis: &'a IctAnalysis,
}

pub(crate) fn build_messages(input: &TechnicalInput<'_>) -> Vec<Message> {
    let last_5: Vec<_> = input.candles.iter().rev().take(5).rev().collect();
    let candles_json: Vec<_> = last_5
        .iter()
        .map(|c| {
            serde_json::json!({
                "open":  c.open.to_string(),
                "high":  c.high.to_string(),
                "low":   c.low.to_string(),
                "close": c.close.to_string(),
            })
        })
        .collect();

    let signal_json = input.analysis.signal.as_ref().map(|s| {
        serde_json::json!({
            "side":  serde_json::to_value(s.side).unwrap_or_default(),
            "entry": s.entry.to_string(),
            "sl":    s.sl.to_string(),
            "tp":    s.tp.to_string(),
        })
    });

    let user_content = serde_json::json!({
        "symbol":           input.symbol,
        "candles":          candles_json,
        "fvgs":             input.analysis.fvgs.len(),
        "order_blocks":     input.analysis.order_blocks.len(),
        "structure_events": input.analysis.structure.len(),
        "sweeps":           input.analysis.sweeps.len(),
        "signal":           signal_json,
    })
    .to_string();

    vec![
        Message {
            role: Role::System,
            content: "You are a technical analyst for FX/crypto trading. \
                      Analyze the given ICT data and price action, then vote Buy, Sell, or Hold."
                .to_string(),
        },
        Message {
            role: Role::User,
            content: user_content,
        },
    ]
}

pub(crate) async fn analyze(
    client: &OpenRouterClient,
    model: &str,
    input: TechnicalInput<'_>,
) -> Result<AgentVote, AgentsError> {
    let messages = build_messages(&input);
    let schema = agent_vote_schema();
    Ok(client.chat(model, messages, &schema).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_candle() -> domain::Candle {
        domain::Candle {
            time: Utc::now(),
            open: "1.1000".parse().unwrap(),
            high: "1.1050".parse().unwrap(),
            low:  "1.0950".parse().unwrap(),
            close: "1.1020".parse().unwrap(),
            tick_volume: 100,
            spread: 10,
            real_volume: 0,
        }
    }

    #[test]
    fn prompt_contains_symbol_and_ict_data() {
        let candles = vec![make_candle(); 3];
        let analysis = ict::IctAnalysis {
            fvgs: vec![],
            order_blocks: vec![],
            structure: vec![],
            sweeps: vec![],
            pd_array: None,
            ote: None,
            signal: None,
        };
        let input = TechnicalInput {
            symbol: "EURUSD",
            candles: &candles,
            analysis: &analysis,
        };
        let messages = build_messages(&input);
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content.contains("EURUSD"));
        assert!(messages[1].content.contains("fvgs"));
        assert!(messages[1].content.contains("order_blocks"));
    }
}
