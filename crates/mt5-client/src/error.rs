#[derive(Debug, thiserror::Error)]
pub enum Mt5Error {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("bridge error ({status}): {detail}")]
    Api { status: u16, detail: String },

    #[error("response parse failed: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("empty response for {endpoint}")]
    Empty { endpoint: String },
}
