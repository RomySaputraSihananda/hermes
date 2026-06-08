use crate::error::OpenRouterError;
use crate::types::{ApiErrorBody, ChatRequest, ChatResponse, JsonSchemaWrapper, Message, ResponseFormat};

#[derive(Default)]
pub struct OpenRouterClient {
    http: reqwest::Client,
}

impl OpenRouterClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn chat<T: serde::de::DeserializeOwned>(
        &self,
        model: &str,
        messages: Vec<Message>,
        schema: &serde_json::Value,
    ) -> Result<T, OpenRouterError> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| OpenRouterError::MissingApiKey)?;

        let request = ChatRequest {
            model,
            messages: &messages,
            response_format: ResponseFormat {
                r#type: "json_schema",
                json_schema: JsonSchemaWrapper {
                    name: "response",
                    strict: true,
                    schema,
                },
            },
        };

        let response = self
            .http
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(&api_key)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if status.as_u16() >= 400 {
            let text = response.text().await?;
            let message = serde_json::from_str::<ApiErrorBody>(&text)
                .map(|b| b.error.message)
                .unwrap_or(text);
            return Err(OpenRouterError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let text = response.text().await?;
        tracing::debug!(model = %model, raw = %text, "openrouter raw response");
        let chat_response: ChatResponse = serde_json::from_str(&text)?;
        let result = parse_response(chat_response)?;
        tracing::debug!(model = %model, "openrouter response ok");
        Ok(result)
    }
}

fn parse_response<T: serde::de::DeserializeOwned>(
    response: ChatResponse,
) -> Result<T, OpenRouterError> {
    if response.choices.is_empty() {
        return Err(OpenRouterError::EmptyChoices);
    }
    Ok(serde_json::from_str(&response.choices[0].message.content)?)
}

#[cfg(test)]
mod tests {
    use super::{parse_response, OpenRouterClient};
    use crate::error::OpenRouterError;
    use crate::types::{ApiErrorBody, ChatResponse, Role};

    #[test]
    fn role_serialization() {
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), r#""system""#);
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), r#""user""#);
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            r#""assistant""#
        );
    }

    #[test]
    fn parse_chat_response() {
        let raw = r#"{
            "id": "gen-abc123",
            "model": "anthropic/claude-3.5-sonnet",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "{\"action\":\"buy\",\"confidence\":0.85}"
                }
            }]
        }"#;
        let response: ChatResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(
            response.choices[0].message.content,
            r#"{"action":"buy","confidence":0.85}"#
        );
    }

    #[test]
    fn parse_api_error_body() {
        let raw = r#"{"error":{"message":"Rate limit exceeded"}}"#;
        let body: ApiErrorBody = serde_json::from_str(raw).unwrap();
        assert_eq!(body.error.message, "Rate limit exceeded");
    }

    #[test]
    fn empty_choices_returns_error() {
        let raw = r#"{"id":"x","model":"x","choices":[]}"#;
        let response: ChatResponse = serde_json::from_str(raw).unwrap();
        let result = parse_response::<serde_json::Value>(response);
        assert!(matches!(result, Err(OpenRouterError::EmptyChoices)));
    }

    #[test]
    fn deserialize_typed_output() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct TradeVote {
            action: String,
            confidence: f64,
        }

        let raw = r#"{
            "id": "gen-abc123",
            "model": "anthropic/claude-3.5-sonnet",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "{\"action\":\"buy\",\"confidence\":0.85}"
                }
            }]
        }"#;
        let response: ChatResponse = serde_json::from_str(raw).unwrap();
        let result: TradeVote = parse_response(response).unwrap();
        assert_eq!(result.action, "buy");
        assert!((result.confidence - 0.85).abs() < f64::EPSILON);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn new_fails_without_api_key() {
        let key = "OPENROUTER_API_KEY";
        let saved = std::env::var(key).ok();
        // SAFETY: current_thread runtime — no concurrent env access in this test
        unsafe { std::env::remove_var(key); }
        let client = OpenRouterClient::new();
        let result = client
            .chat::<serde_json::Value>("test-model", vec![], &serde_json::json!({}))
            .await;
        if let Some(v) = saved {
            unsafe { std::env::set_var(key, v); }
        }
        assert!(matches!(result, Err(OpenRouterError::MissingApiKey)));
    }
}
