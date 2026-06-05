#[derive(Debug, thiserror::Error)]
pub enum OpenRouterError {
    #[error("OPENROUTER_API_KEY not set")]
    MissingApiKey,

    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("api error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("response parse failed: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("empty choices in response")]
    EmptyChoices,
}
