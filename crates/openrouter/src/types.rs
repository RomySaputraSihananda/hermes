use serde::{Deserialize, Serialize};

// --- Public types ---

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

// --- Internal request types ---

#[derive(Serialize)]
pub(crate) struct ChatRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) messages: &'a [Message],
    pub(crate) response_format: ResponseFormat<'a>,
}

#[derive(Serialize)]
pub(crate) struct ResponseFormat<'a> {
    pub(crate) r#type: &'static str,
    pub(crate) json_schema: JsonSchemaWrapper<'a>,
}

#[derive(Serialize)]
pub(crate) struct JsonSchemaWrapper<'a> {
    pub(crate) name: &'static str,
    pub(crate) strict: bool,
    pub(crate) schema: &'a serde_json::Value,
}

// --- Internal response types ---

#[derive(Deserialize)]
pub(crate) struct ChatResponse {
    pub(crate) choices: Vec<Choice>,
}

#[derive(Deserialize)]
pub(crate) struct Choice {
    pub(crate) message: AssistantMessage,
}

#[derive(Deserialize)]
pub(crate) struct AssistantMessage {
    pub(crate) content: String,
}

// --- Internal error body ---

#[derive(Deserialize)]
pub(crate) struct ApiErrorBody {
    pub(crate) error: ApiErrorDetail,
}

#[derive(Deserialize)]
pub(crate) struct ApiErrorDetail {
    pub(crate) message: String,
}
