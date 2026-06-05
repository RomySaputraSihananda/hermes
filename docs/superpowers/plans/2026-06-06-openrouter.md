# openrouter Crate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `crates/openrouter` — a thin async HTTP client for the OpenRouter API with a generic `chat<T>()` method that POSTs structured-output requests and returns `T: DeserializeOwned`.

**Architecture:** 4-file crate: `error.rs` defines `OpenRouterError`, `types.rs` holds public `Message`/`Role` and private request/response types, `client.rs` contains `OpenRouterClient` with `new()` + `chat<T>()` + all 6 offline tests, and `lib.rs` re-exports the public surface. `chat<T>()` reads `OPENROUTER_API_KEY` from the environment on each call, builds a `response_format.json_schema` request body, and deserializes `choices[0].message.content` into `T`.

**Tech Stack:** reqwest 0.12 (rustls-tls + json), serde 1 + serde_json 1, thiserror 2, tokio 1 (full), tracing 0.1.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/openrouter/Cargo.toml` | Modify | Add `serde = { workspace = true }` |
| `crates/openrouter/src/error.rs` | Create | `OpenRouterError` (5 variants) |
| `crates/openrouter/src/types.rs` | Create | `Message`, `Role` (pub); 8 internal types (pub(crate)) |
| `crates/openrouter/src/client.rs` | Create | `OpenRouterClient`, `parse_response<T>`, `chat<T>`, all 6 tests |
| `crates/openrouter/src/lib.rs` | Modify | Module declarations + `pub use` |

---

### Task 1: Cargo.toml + error.rs + lib.rs initial scaffold

**Files:**
- Modify: `crates/openrouter/Cargo.toml`
- Create: `crates/openrouter/src/error.rs`
- Modify: `crates/openrouter/src/lib.rs`

- [ ] **Step 1: Add serde to Cargo.toml**

Replace the full contents of `crates/openrouter/Cargo.toml`:

```toml
[package]
name    = "openrouter"
version = "0.1.0"
edition = "2024"

[dependencies]
reqwest    = { workspace = true }
serde      = { workspace = true }
serde_json = { workspace = true }
thiserror  = { workspace = true }
tokio      = { workspace = true }
tracing    = { workspace = true }
```

- [ ] **Step 2: Create error.rs**

Create `crates/openrouter/src/error.rs`:

```rust
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
```

- [ ] **Step 3: Update lib.rs (initial — only error for now)**

Replace `crates/openrouter/src/lib.rs`:

```rust
mod error;

pub use error::OpenRouterError;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p openrouter
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/openrouter/Cargo.toml crates/openrouter/src/error.rs crates/openrouter/src/lib.rs
git commit -m "feat(openrouter): add error types and serde dependency"
```

---

### Task 2: types.rs — all public and internal types

**Files:**
- Create: `crates/openrouter/src/types.rs`
- Modify: `crates/openrouter/src/lib.rs`

- [ ] **Step 1: Create types.rs with all types**

Create `crates/openrouter/src/types.rs`:

```rust
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
```

- [ ] **Step 2: Expand lib.rs to include types**

Replace `crates/openrouter/src/lib.rs`:

```rust
mod error;
mod types;

pub use error::OpenRouterError;
pub use types::{Message, Role};
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p openrouter
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/openrouter/src/types.rs crates/openrouter/src/lib.rs
git commit -m "feat(openrouter): add Message, Role, and internal request/response types"
```

---

### Task 3: client.rs — 3 type deserialization tests

These tests live in `client.rs` and verify that the types defined in `types.rs` serialize/deserialize correctly. They don't require `OpenRouterClient` to exist yet.

**Files:**
- Create: `crates/openrouter/src/client.rs`
- Modify: `crates/openrouter/src/lib.rs`

- [ ] **Step 1: Write the 3 failing tests in client.rs**

Create `crates/openrouter/src/client.rs`:

```rust
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
```

- [ ] **Step 2: Add mod client to lib.rs**

Replace `crates/openrouter/src/lib.rs`:

```rust
mod client;
mod error;
mod types;

pub use error::OpenRouterError;
pub use types::{Message, Role};
```

(No `pub use client::*` yet — nothing public in client.rs.)

- [ ] **Step 3: Run the 3 tests to confirm they compile and pass**

```bash
cargo test -p openrouter -- role_serialization parse_chat_response parse_api_error_body --no-fail-fast
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/openrouter/src/client.rs crates/openrouter/src/lib.rs
git commit -m "test(openrouter): add role serialization and response parsing tests"
```

---

### Task 4: client.rs — parse_response helper + 2 more tests

`parse_response<T>` extracts `T` from a `ChatResponse`, handling the empty-choices edge case. Two new tests drive its implementation.

**Files:**
- Modify: `crates/openrouter/src/client.rs`

- [ ] **Step 1: Write 2 failing tests (add to the existing tests mod)**

Add these two tests inside the `mod tests { ... }` block in `crates/openrouter/src/client.rs`. The full file now looks like:

```rust
fn parse_response<T: serde::de::DeserializeOwned>(
    response: crate::types::ChatResponse,
) -> Result<T, crate::error::OpenRouterError> {
    // placeholder — will be implemented in Step 3
    let _ = response;
    unimplemented!()
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
```

- [ ] **Step 2: Run the 2 new tests to confirm they fail**

```bash
cargo test -p openrouter -- empty_choices_returns_error deserialize_typed_output --no-fail-fast 2>&1 | head -20
```

Expected: panics with "not implemented".

- [ ] **Step 3: Implement parse_response**

Replace the placeholder `parse_response` at the top of `client.rs` with the real implementation:

```rust
fn parse_response<T: serde::de::DeserializeOwned>(
    response: crate::types::ChatResponse,
) -> Result<T, crate::error::OpenRouterError> {
    if response.choices.is_empty() {
        return Err(crate::error::OpenRouterError::EmptyChoices);
    }
    Ok(serde_json::from_str(&response.choices[0].message.content)?)
}
```

- [ ] **Step 4: Run all 5 tests to confirm they pass**

```bash
cargo test -p openrouter
```

Expected: 5 tests pass, 0 failures.

- [ ] **Step 5: Commit**

```bash
git add crates/openrouter/src/client.rs
git commit -m "feat(openrouter): add parse_response helper with empty-choices guard"
```

---

### Task 5: OpenRouterClient + chat\<T\>() + final wiring

**Files:**
- Modify: `crates/openrouter/src/client.rs`
- Modify: `crates/openrouter/src/lib.rs`

- [ ] **Step 1: Write the failing new_fails_without_api_key test**

Add this test to the `mod tests` block inside `client.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to confirm it fails (compile error — OpenRouterClient not defined)**

```bash
cargo test -p openrouter -- new_fails_without_api_key 2>&1 | head -20
```

Expected: compile error "cannot find struct `OpenRouterClient`".

- [ ] **Step 3: Implement OpenRouterClient and chat\<T\>()**

Replace the full contents of `crates/openrouter/src/client.rs` with the complete implementation:

```rust
use crate::error::OpenRouterError;
use crate::types::{ApiErrorBody, ChatRequest, ChatResponse, JsonSchemaWrapper, Message, ResponseFormat};

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
            .await
            .map_err(OpenRouterError::Http)?;

        let status = response.status();
        if status.as_u16() >= 400 {
            let text = response.text().await.map_err(OpenRouterError::Http)?;
            let message = serde_json::from_str::<ApiErrorBody>(&text)
                .map(|b| b.error.message)
                .unwrap_or(text);
            return Err(OpenRouterError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let text = response.text().await.map_err(OpenRouterError::Http)?;
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
```

- [ ] **Step 4: Update lib.rs to expose OpenRouterClient**

Replace `crates/openrouter/src/lib.rs`:

```rust
mod client;
mod error;
mod types;

pub use client::OpenRouterClient;
pub use error::OpenRouterError;
pub use types::{Message, Role};
```

- [ ] **Step 5: Run all 6 tests**

```bash
cargo test -p openrouter
```

Expected output contains:
```
test tests::role_serialization ... ok
test tests::parse_chat_response ... ok
test tests::parse_api_error_body ... ok
test tests::empty_choices_returns_error ... ok
test tests::deserialize_typed_output ... ok
test tests::new_fails_without_api_key ... ok

test result: ok. 6 passed; 0 failed
```

- [ ] **Step 6: Run clippy with zero-warnings gate**

```bash
cargo clippy -p openrouter -- -D warnings
```

Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/openrouter/src/client.rs crates/openrouter/src/lib.rs
git commit -m "feat(openrouter): implement OpenRouterClient with chat<T> and 6 offline tests"
```

---

## Success Criteria

- `cargo test -p openrouter` → 6 tests pass
- `cargo clippy -p openrouter -- -D warnings` → zero warnings
- `use openrouter::{OpenRouterClient, OpenRouterError, Message, Role}` compiles
- No panic paths — `MissingApiKey` returned as error, never panics
