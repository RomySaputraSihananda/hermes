fn parse_response<T: serde::de::DeserializeOwned>(
    response: crate::types::ChatResponse,
) -> Result<T, crate::error::OpenRouterError> {
    if response.choices.is_empty() {
        return Err(crate::error::OpenRouterError::EmptyChoices);
    }
    Ok(serde_json::from_str(&response.choices[0].message.content)?)
}

#[cfg(test)]
mod tests {
    use super::parse_response;
    use crate::types::{ApiErrorBody, ChatResponse, Role};
    use crate::error::OpenRouterError;

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
}
