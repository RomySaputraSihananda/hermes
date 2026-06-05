use crate::error::AgentsError;
use crate::types::{AgentDecision, AgentVote, agent_vote_schema};
use openrouter::{Message, OpenRouterClient, Role};

pub(crate) fn build_orchestrator_messages(votes: &[AgentVote; 4]) -> Vec<Message> {
    let content = serde_json::json!({
        "technical":   votes[0],
        "sentiment":   votes[1],
        "fundamental": votes[2],
        "risk":        votes[3],
    })
    .to_string();

    vec![
        Message {
            role: Role::System,
            content: "You are a trading orchestrator. Given votes from 4 agents \
                      (technical, sentiment, fundamental, risk), return the best action."
                .to_string(),
        },
        Message {
            role: Role::User,
            content,
        },
    ]
}

pub(crate) async fn run(
    client: &OpenRouterClient,
    model: &str,
    votes: [AgentVote; 4],
) -> Result<AgentDecision, AgentsError> {
    // Unanimous quorum: all 4 votes agree — skip LLM
    if votes[1].action == votes[0].action
        && votes[2].action == votes[0].action
        && votes[3].action == votes[0].action
    {
        let confidence = votes.iter().map(|v| v.confidence).sum::<f64>() / 4.0;
        let action = votes[0].action.clone();
        return Ok(AgentDecision {
            action,
            confidence,
            reasoning: "unanimous quorum".to_string(),
            votes,
            from_quorum: true,
        });
    }

    // Non-unanimous: call LLM orchestrator
    let messages = build_orchestrator_messages(&votes);
    let schema = agent_vote_schema();
    let vote: AgentVote = client.chat(model, messages, &schema).await?;
    tracing::debug!(
        from_quorum = false,
        action = ?vote.action,
        "orchestrator decided"
    );
    Ok(AgentDecision {
        action: vote.action,
        confidence: vote.confidence,
        reasoning: vote.reasoning,
        votes,
        from_quorum: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Action;

    fn make_vote(action: Action) -> crate::types::AgentVote {
        crate::types::AgentVote {
            action,
            confidence: 0.8,
            reasoning: "test".to_string(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unanimous_returns_quorum() {
        let votes = [
            make_vote(Action::Buy),
            make_vote(Action::Buy),
            make_vote(Action::Buy),
            make_vote(Action::Buy),
        ];
        let client = openrouter::OpenRouterClient::new();
        let decision = run(&client, "test-model", votes).await.unwrap();
        assert!(decision.from_quorum);
        assert_eq!(decision.action, Action::Buy);
        assert!((decision.confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn non_unanimous_builds_correct_prompt() {
        let votes = [
            make_vote(Action::Buy),
            make_vote(Action::Hold),
            make_vote(Action::Buy),
            make_vote(Action::Buy),
        ];
        let messages = build_orchestrator_messages(&votes);
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content.contains("technical"));
        assert!(messages[1].content.contains("sentiment"));
        assert!(messages[1].content.contains("fundamental"));
        assert!(messages[1].content.contains("risk"));
    }
}
