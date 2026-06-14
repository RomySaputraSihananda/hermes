use crate::error::AgentsError;
use crate::types::{AgentVote, agent_vote_schema};
use domain::Candle;
use openrouter::{Message, OpenRouterClient, Role};

#[derive(Copy, Clone)]
pub struct SentimentInput<'a> {
    pub symbol: &'a str,
    pub candles: &'a [Candle],
}

pub(crate) fn build_messages(input: &SentimentInput<'_>) -> Vec<Message> {
    let last_10: Vec<_> = input.candles.iter().rev().take(10).rev().collect();
    let candles_json: Vec<_> = last_10
        .iter()
        .map(|c| {
            serde_json::json!({
                "open":   c.open.to_string(),
                "high":   c.high.to_string(),
                "low":    c.low.to_string(),
                "close":  c.close.to_string(),
                "volume": c.tick_volume,
            })
        })
        .collect();

    let user_content = serde_json::json!({
        "symbol":  input.symbol,
        "candles": candles_json,
    })
    .to_string();

    vec![
        Message {
            role: Role::System,
            content: "You are a market sentiment analyst. \
                      Infer market sentiment from recent price action for the given symbol. \
                      Respond ONLY with a raw JSON object, no markdown, no explanation: \
                      {\"action\":\"buy|sell|hold\",\"confidence\":0.0-1.0,\"reasoning\":\"...\"}."
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
    input: SentimentInput<'_>,
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
            open: "59000".parse().unwrap(),
            high: "59500".parse().unwrap(),
            low:  "58800".parse().unwrap(),
            close: "59200".parse().unwrap(),
            tick_volume: 500,
            spread: 100,
            real_volume: 0,
        }
    }

    #[test]
    fn prompt_contains_symbol_and_price_action() {
        let candles = vec![make_candle(); 5];
        let input = SentimentInput { symbol: "BTCUSD", candles: &candles };
        let messages = build_messages(&input);
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content.contains("BTCUSD"));
        assert!(messages[1].content.contains("candles"));
    }
}
