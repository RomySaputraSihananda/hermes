use crate::error::AgentsError;
use crate::types::{AgentVote, agent_vote_schema};
use openrouter::{Message, OpenRouterClient, Role};

pub struct FundamentalInput<'a> {
    pub symbol: &'a str,
    pub news_context: &'a str,
}

pub(crate) fn build_messages(input: &FundamentalInput<'_>) -> Vec<Message> {
    let user_content = serde_json::json!({
        "symbol":       input.symbol,
        "news_context": input.news_context,
    })
    .to_string();

    vec![
        Message {
            role: Role::System,
            content: "You are a fundamental analyst for FX/crypto markets. \
                      Given the symbol and any available news context, \
                      vote on the fundamental outlook. \
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
    input: FundamentalInput<'_>,
) -> Result<AgentVote, AgentsError> {
    let messages = build_messages(&input);
    let schema = agent_vote_schema();
    Ok(client.chat(model, messages, &schema).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_symbol_and_news() {
        let input = FundamentalInput {
            symbol: "GBPUSD",
            news_context: "UK GDP rose 0.3% in Q1",
        };
        let messages = build_messages(&input);
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content.contains("GBPUSD"));
        assert!(messages[1].content.contains("UK GDP rose 0.3%"));
    }
}
