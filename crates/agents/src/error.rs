#[derive(Debug, thiserror::Error)]
pub enum AgentsError {
    #[error("openrouter error: {0}")]
    OpenRouter(#[from] openrouter::OpenRouterError),

    #[error("invalid action from agent: {0}")]
    Parse(String),
}
