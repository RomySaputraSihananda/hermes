#[cfg(test)]
mod tests {
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
}
