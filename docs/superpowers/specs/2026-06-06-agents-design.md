# agents Crate — Design Spec

**Date:** 2026-06-06
**Phase:** 5
**Status:** Approved

---

## Tujuan

Implementasikan `crates/agents` — crate yang menjalankan 4 LLM agent (technical, sentiment, fundamental, risk) secara paralel, mengumpulkan vote masing-masing, lalu melalui orchestrator menghasilkan `AgentDecision` final.

Digunakan oleh Phase 7 (`engine` crate) untuk menambah lapisan LLM reasoning di atas sinyal ICT dari Phase 3.

---

## Dependency

```toml
[dependencies]
domain     = { workspace = true }
ict        = { workspace = true }   # IctAnalysis, TradeSignal
openrouter = { workspace = true }   # OpenRouterClient, OpenRouterError
serde      = { workspace = true }   # derive Serialize/Deserialize
serde_json = { workspace = true }   # JSON schema + prompt building
thiserror  = { workspace = true }   # error derivation
tokio      = { workspace = true }   # async runtime + join!
tracing    = { workspace = true }   # structured logging
```

`ict = { workspace = true }` dan `serde = { workspace = true }` perlu ditambahkan ke `crates/agents/Cargo.toml` (belum ada di scaffold).

---

## File Structure

```
crates/agents/src/
├── lib.rs          — pub use + pub async fn run_agents(...)
├── error.rs        — AgentsError
├── types.rs        — Action, AgentVote, AgentDecision
├── technical.rs    — TechnicalInput + pub(crate) analyze()
├── sentiment.rs    — SentimentInput + pub(crate) analyze()
├── fundamental.rs  — FundamentalInput + pub(crate) analyze()
├── risk.rs         — RiskInput + pub(crate) analyze()
└── orchestrator.rs — pub(crate) run() dengan quorum + LLM fallback
```

---

## Shared Types (`types.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentVote {
    pub action: Action,
    pub confidence: f64,   // 0.0 – 1.0
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub struct AgentDecision {
    pub action: Action,
    pub confidence: f64,
    pub reasoning: String,
    pub votes: [AgentVote; 4],   // [technical, sentiment, fundamental, risk]
    pub from_quorum: bool,       // true = unanimous shortcut, false = LLM decided
}
```

**JSON schema untuk semua agent** (structured output OpenRouter):

```json
{
    "type": "object",
    "properties": {
        "action":     { "type": "string", "enum": ["buy", "sell", "hold"] },
        "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
        "reasoning":  { "type": "string" }
    },
    "required": ["action", "confidence", "reasoning"],
    "additionalProperties": false
}
```

Schema ini dikembalikan oleh `pub(crate) fn agent_vote_schema() -> serde_json::Value` di `types.rs` dan dipakai oleh semua 5 agent (4 individual + orchestrator).

---

## Error Type (`error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum AgentsError {
    #[error("openrouter error: {0}")]
    OpenRouter(#[from] openrouter::OpenRouterError),

    #[error("invalid action from agent: {0}")]
    Parse(String),
}
```

---

## Agent Input Structs & Functions

Semua fungsi `analyze` adalah `pub(crate)` — hanya dipanggil dari `lib.rs` via `run_agents`.

`OpenRouterClient` dan `model: &str` di-pass dari caller sehingga dapat di-reuse di semua agent tanpa re-construct.

### `technical.rs`

```rust
pub struct TechnicalInput<'a> {
    pub symbol: &'a str,
    pub candles: &'a [domain::Candle],
    pub analysis: &'a ict::IctAnalysis,
}

pub(crate) async fn analyze(
    client: &openrouter::OpenRouterClient,
    model: &str,
    input: TechnicalInput<'_>,
) -> Result<AgentVote, AgentsError>;
```

**Prompt system:** `"You are a technical analyst for FX/crypto trading. Analyze the given ICT data and price action, then vote Buy, Sell, or Hold."`

**Prompt user:** JSON dari `symbol`, ringkasan `analysis` (BOS side, OTE range, signal entry/sl/tp jika ada), 5 candle terakhir (OHLC).

### `sentiment.rs`

```rust
pub struct SentimentInput<'a> {
    pub symbol: &'a str,
    pub candles: &'a [domain::Candle],
}

pub(crate) async fn analyze(
    client: &openrouter::OpenRouterClient,
    model: &str,
    input: SentimentInput<'_>,
) -> Result<AgentVote, AgentsError>;
```

**Prompt system:** `"You are a market sentiment analyst. Infer market sentiment from recent price action for the given symbol."`

**Prompt user:** JSON dari `symbol` + 10 candle terakhir (OHLC + volume).

### `fundamental.rs`

```rust
pub struct FundamentalInput<'a> {
    pub symbol: &'a str,
    pub news_context: &'a str,   // caller-provided text, boleh kosong ""
}

pub(crate) async fn analyze(
    client: &openrouter::OpenRouterClient,
    model: &str,
    input: FundamentalInput<'_>,
) -> Result<AgentVote, AgentsError>;
```

**Prompt system:** `"You are a fundamental analyst for FX/crypto markets. Given the symbol and any available news context, vote on the fundamental outlook."`

**Prompt user:** JSON dari `symbol` + `news_context`.

### `risk.rs`

```rust
pub struct RiskInput<'a> {
    pub account: &'a domain::AccountInfo,
    pub positions: &'a [domain::Position],
    pub signal: &'a ict::TradeSignal,
}

pub(crate) async fn analyze(
    client: &openrouter::OpenRouterClient,
    model: &str,
    input: RiskInput<'_>,
) -> Result<AgentVote, AgentsError>;
```

**Prompt system:** `"You are a risk manager for a trading bot. Given account info, open positions, and a proposed trade signal, assess whether the trade is safe to execute."`

**Prompt user:** JSON dari account balance/equity, jumlah open positions, dan signal entry/sl/tp.

---

## Orchestrator (`orchestrator.rs`)

```rust
pub(crate) async fn run(
    client: &openrouter::OpenRouterClient,
    model: &str,
    votes: [AgentVote; 4],   // [technical, sentiment, fundamental, risk]
) -> Result<AgentDecision, AgentsError>;
```

**Logic:**

```
1. Cek unanimous: jika votes[0..4].action semua sama
   → return AgentDecision {
         action:       votes[0].action.clone(),
         confidence:   votes.iter().map(|v| v.confidence).sum::<f64>() / 4.0,
         reasoning:    "unanimous quorum".to_string(),
         votes,
         from_quorum:  true,
     }

2. Jika tidak unanimous → call LLM orchestrator:
   System: "You are a trading orchestrator. Given votes from 4 agents
            (technical, sentiment, fundamental, risk), return the best action."
   User:   JSON object dengan label per-agent:
           {
             "technical":   votes[0],
             "sentiment":   votes[1],
             "fundamental": votes[2],
             "risk":        votes[3]
           }
   Schema: agent_vote_schema()   ← same schema as individual agents
   → parse AgentVote { action, confidence, reasoning }
   → return AgentDecision { action, confidence, reasoning, votes, from_quorum: false }
```

**`tracing::debug!`** dipanggil setelah keputusan final: `debug!(from_quorum = %decision.from_quorum, action = ?decision.action, "orchestrator decided")`.

---

## Public Entry Point (`lib.rs`)

```rust
pub async fn run_agents(
    client: &openrouter::OpenRouterClient,
    model: &str,
    technical: technical::TechnicalInput<'_>,
    sentiment: sentiment::SentimentInput<'_>,
    fundamental: fundamental::FundamentalInput<'_>,
    risk: risk::RiskInput<'_>,
) -> Result<AgentDecision, AgentsError>;
```

**Implementasi:**

```rust
let (t, s, f, r) = tokio::try_join!(
    technical::analyze(client, model, technical),
    sentiment::analyze(client, model, sentiment),
    fundamental::analyze(client, model, fundamental),
    risk::analyze(client, model, risk),
)?;
let votes = [t, s, f, r];
orchestrator::run(client, model, votes).await
```

**Public surface dari `lib.rs`:**

```rust
pub use error::AgentsError;
pub use types::{Action, AgentDecision, AgentVote};
pub use technical::TechnicalInput;
pub use sentiment::SentimentInput;
pub use fundamental::FundamentalInput;
pub use risk::RiskInput;
```

---

## Testing

Inline `#[cfg(test)]` di setiap file. Semua test offline.

| File | Test | Assertion |
|------|------|-----------|
| `technical.rs` | `prompt_contains_symbol_and_ict_data` | User message JSON mengandung `symbol` dan setidaknya satu field dari IctAnalysis |
| `sentiment.rs` | `prompt_contains_symbol_and_price_action` | User message mengandung `symbol` dan candle OHLC |
| `fundamental.rs` | `prompt_contains_symbol_and_news` | User message mengandung `symbol` dan `news_context` |
| `risk.rs` | `prompt_contains_account_and_signal` | User message mengandung account balance dan signal entry |
| `orchestrator.rs` | `unanimous_returns_quorum` | 4 identical votes → `from_quorum: true`, confidence = avg |
| `orchestrator.rs` | `non_unanimous_builds_correct_prompt` | Mixed votes → user message JSON mengandung semua 4 votes |

**Catatan testing:** Karena semua agent memanggil OpenRouter secara async, test-test di atas hanya menguji **prompt building** (sisi data → string) secara sinkron, bukan actual HTTP call. Setiap agent mengekspos `pub(crate) fn build_messages(input) -> Vec<Message>` helper yang dapat ditest tanpa `OpenRouterClient`.

---

## Success Criteria

- `cargo test -p agents` → semua 6 tests hijau
- `cargo clippy -p agents -- -D warnings` → zero warnings
- `use agents::{run_agents, AgentDecision, AgentVote, Action, TechnicalInput, SentimentInput, FundamentalInput, RiskInput}` — semua accessible
- `run_agents` memanggil 4 agent secara parallel (`tokio::try_join!`)
- Orchestrator: unanimous → `from_quorum: true` tanpa LLM call; tidak unanimous → LLM dipanggil
