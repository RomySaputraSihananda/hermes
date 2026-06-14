mod error;
mod fundamental;
mod orchestrator;
mod risk;
mod sentiment;
mod technical;
mod types;

pub use error::AgentsError;
pub use fundamental::FundamentalInput;
pub use risk::RiskInput;
pub use sentiment::SentimentInput;
pub use technical::TechnicalInput;
pub use types::{Action, AgentDecision, AgentVote};

pub async fn run_agents(
    client: &openrouter::OpenRouterClient,
    model: &str,
    technical_in: TechnicalInput<'_>,
    sentiment_in: SentimentInput<'_>,
    fundamental_in: FundamentalInput<'_>,
    risk_in: RiskInput<'_>,
) -> Result<AgentDecision, AgentsError> {
    // Retry once on transient failures (parse errors, network hiccups).
    for attempt in 0..2u8 {
        match tokio::try_join!(
            technical::analyze(client, model, technical_in),
            sentiment::analyze(client, model, sentiment_in),
            fundamental::analyze(client, model, fundamental_in),
            risk::analyze(client, model, risk_in),
        ) {
            Ok((t, s, f, r)) => return orchestrator::run(client, model, [t, s, f, r]).await,
            Err(e) if attempt == 0 => {
                tracing::warn!(error = %e, "agents attempt 1 failed, retrying");
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
