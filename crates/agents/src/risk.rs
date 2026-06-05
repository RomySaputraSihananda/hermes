use crate::error::AgentsError;
use crate::types::{AgentVote, agent_vote_schema};
use domain::{AccountInfo, Position};
use ict::TradeSignal;
use openrouter::{Message, OpenRouterClient, Role};

pub struct RiskInput<'a> {
    pub account: &'a AccountInfo,
    pub positions: &'a [Position],
    pub signal: &'a TradeSignal,
}

pub(crate) fn build_messages(input: &RiskInput<'_>) -> Vec<Message> {
    let user_content = serde_json::json!({
        "account": {
            "balance":     input.account.balance.to_string(),
            "equity":      input.account.equity.to_string(),
            "margin_free": input.account.margin_free.to_string(),
            "currency":    input.account.currency,
        },
        "open_positions": input.positions.len(),
        "signal": {
            "side":  serde_json::to_value(input.signal.side).unwrap_or_default(),
            "entry": input.signal.entry.to_string(),
            "sl":    input.signal.sl.to_string(),
            "tp":    input.signal.tp.to_string(),
        },
    })
    .to_string();

    vec![
        Message {
            role: Role::System,
            content: "You are a risk manager for a trading bot. \
                      Given account info, open positions, and a proposed trade signal, \
                      assess whether the trade is safe to execute."
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
    input: RiskInput<'_>,
) -> Result<AgentVote, AgentsError> {
    let messages = build_messages(&input);
    let schema = agent_vote_schema();
    Ok(client.chat(model, messages, &schema).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_account() -> domain::AccountInfo {
        domain::AccountInfo {
            login: 123456,
            leverage: 100,
            trade_allowed: true,
            trade_expert: true,
            currency: "USD".to_string(),
            currency_digits: 2,
            server: "TestServer".to_string(),
            name: "Test".to_string(),
            company: "TestCo".to_string(),
            balance:      "5000".parse().unwrap(),
            equity:       "5000".parse().unwrap(),
            profit:       "0".parse().unwrap(),
            credit:       "0".parse().unwrap(),
            margin:       "0".parse().unwrap(),
            margin_free:  "5000".parse().unwrap(),
            margin_level: "0".parse().unwrap(),
        }
    }

    fn make_signal() -> ict::TradeSignal {
        ict::TradeSignal {
            side: domain::Side::Long,
            entry: "1.1000".parse().unwrap(),
            sl:    "1.0950".parse().unwrap(),
            tp:    "1.1100".parse().unwrap(),
            confluence: ict::ConfluenceFlags::default(),
        }
    }

    #[test]
    fn prompt_contains_account_and_signal() {
        let account = make_account();
        let signal = make_signal();
        let input = RiskInput {
            account: &account,
            positions: &[],
            signal: &signal,
        };
        let messages = build_messages(&input);
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content.contains("5000"));
        assert!(messages[1].content.contains("1.1000"));
    }
}
