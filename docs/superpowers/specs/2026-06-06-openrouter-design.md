# openrouter Crate — Design Spec

**Date:** 2026-06-06
**Phase:** 4
**Status:** Approved

---

## Tujuan

Implementasikan `crates/openrouter` — thin async HTTP client untuk OpenRouter API (OpenAI-compatible). Crate mengekspos satu method generik `chat<T>` yang mengirim messages + JSON schema ke model LLM pilihan dan mengembalikan `T: DeserializeOwned` langsung.

Digunakan oleh Phase 5 (`agents` crate) untuk 5 role agent: technical analyst, sentiment, fundamental, risk, dan orchestrator.

---

## Dependency

```toml
[dependencies]
reqwest    = { workspace = true }   # HTTP client (rustls-tls + json)
serde      = { workspace = true }   # derive Serialize/Deserialize
serde_json = { workspace = true }   # JSON schema + parse
thiserror  = { workspace = true }   # error derivation
tokio      = { workspace = true }   # async runtime
tracing    = { workspace = true }   # structured logging
```

`serde = { workspace = true }` perlu ditambahkan ke `crates/openrouter/Cargo.toml` (belum ada di scaffold).

---

## File Structure

```
crates/openrouter/src/
├── lib.rs      — pub use OpenRouterClient, OpenRouterError, Message, Role
├── error.rs    — OpenRouterError enum
├── types.rs    — Message, Role (pub); ChatRequest, ChatResponse, ApiErrorBody (private)
└── client.rs   — OpenRouterClient struct + chat<T> method + tests
```

---

## Public API

### `OpenRouterClient`

```rust
pub struct OpenRouterClient {
    api_key: String,
    http: reqwest::Client,
}

impl OpenRouterClient {
    /// Baca OPENROUTER_API_KEY dari environment. Error jika tidak ada.
    pub fn new() -> Result<Self, OpenRouterError>;

    pub async fn chat<T: serde::de::DeserializeOwned>(
        &self,
        model: &str,
        messages: Vec<Message>,
        schema: &serde_json::Value,
    ) -> Result<T, OpenRouterError>;
}
```

### `Message` & `Role`

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}
```

---

## Error Types (`error.rs`)

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

---

## Internal Types (private di `types.rs`)

```rust
// Request body
#[derive(serde::Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    response_format: ResponseFormat<'a>,
}

#[derive(serde::Serialize)]
struct ResponseFormat<'a> {
    r#type: &'static str,            // selalu "json_schema"
    json_schema: JsonSchemaWrapper<'a>,
}

#[derive(serde::Serialize)]
struct JsonSchemaWrapper<'a> {
    name: &'static str,              // selalu "response"
    strict: bool,                    // selalu true
    schema: &'a serde_json::Value,
}

// Response body
#[derive(serde::Deserialize)]
struct ChatResponse { choices: Vec<Choice> }

#[derive(serde::Deserialize)]
struct Choice { message: AssistantMessage }

#[derive(serde::Deserialize)]
struct AssistantMessage { content: String }

// Error body
#[derive(serde::Deserialize)]
struct ApiErrorBody { error: ApiErrorDetail }

#[derive(serde::Deserialize)]
struct ApiErrorDetail { message: String }
```

---

## `chat<T>` Implementation Flow

```
Base URL: "https://openrouter.ai/api/v1/chat/completions"

1. std::env::var("OPENROUTER_API_KEY") → Err → MissingApiKey
2. Build ChatRequest { model, messages, response_format: { type: "json_schema", json_schema: { name: "response", strict: true, schema } } }
3. POST request dengan header Authorization: Bearer <api_key>
4. Network error → Http
5. HTTP status ≥ 400 → parse ApiErrorBody → Api { status, message }
6. .text() → serde_json::from_str::<ChatResponse> → gagal → Parse
7. choices kosong → EmptyChoices
8. serde_json::from_str::<T>(&choices[0].message.content) → gagal → Parse
9. tracing::debug!(model = %model, "openrouter response ok")
10. Return T
```

**Catatan:** `OPENROUTER_API_KEY` dibaca setiap kali `chat()` dipanggil (bukan saat `new()`), sehingga nilai env var bisa berubah tanpa re-construct client. Ini berguna untuk testing dengan `std::env::set_var`.

---

## Logging

`tracing::debug!(model = %model, "openrouter response ok")` per successful call. Tidak log API key atau content response.

---

## Testing

Inline `#[cfg(test)]` di `client.rs`. Semua test offline — tidak butuh server hidup.

| Test | Assertion utama |
|------|----------------|
| `parse_chat_response` | Parse raw OpenRouter JSON response, `content` field benar |
| `deserialize_typed_output` | Content string → `TradeVote { action: String, confidence: f64 }` |
| `parse_api_error_body` | Parse `{"error":{"message":"Rate limit exceeded"}}` → `ApiErrorDetail.message` benar |
| `role_serialization` | `Role::System` → `"system"`, `Role::User` → `"user"` |
| `empty_choices_returns_error` | Response dengan `"choices":[]` → `EmptyChoices` variant |
| `new_fails_without_api_key` | Unset env var → `Err(OpenRouterError::MissingApiKey)` |

**JSON literal untuk `parse_chat_response` dan `deserialize_typed_output`:**

```rust
// OpenRouter response format
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
```

---

## Success Criteria

- `cargo test -p openrouter` → semua 6 tests hijau
- `cargo clippy -p openrouter -- -D warnings` → zero warnings
- `use openrouter::{OpenRouterClient, OpenRouterError, Message, Role}` — semua accessible
- Tidak ada panic path — `MissingApiKey` returned sebagai error, bukan panic
