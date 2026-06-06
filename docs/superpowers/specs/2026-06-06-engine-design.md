# engine Crate — Design Spec

**Date:** 2026-06-06
**Phase:** 7
**Status:** Approved

---

## Tujuan

Implementasikan `crates/engine` — orchestrator utama trading pipeline hermes. Engine menerima daftar symbol, menjalankan analisis ICT + AI agents + risk management secara paralel untuk setiap symbol, memilih kandidat terbaik, lalu mengeksekusi trade ke MT5.

Digunakan oleh `bin/hermes` sebagai entry point utama setiap siklus trading.

---

## Dependency

```toml
[dependencies]
agents       = { workspace = true }
domain       = { workspace = true }
ict          = { workspace = true }
mt5-client   = { workspace = true }
openrouter   = { workspace = true }
risk         = { workspace = true }
rust_decimal = { workspace = true }
thiserror    = { workspace = true }
tokio        = { workspace = true }
tracing      = { workspace = true }
```

---

## File Structure

```
crates/engine/src/
├── lib.rs      — pub run_once, EngineConfig, EngineOutcome, EngineError, helpers
└── execute.rs  — build_trade_request(), execute_trade()

crates/mt5-client/src/
├── types.rs    — tambah TradeResult
└── client.rs   — tambah place_order()
```

---

## mt5-client Additions

### `TradeResult` (types.rs)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct TradeResult {
    pub retcode: u32,
    pub order: u64,     // MT5 order ticket
    pub comment: String,
}
```

### `place_order` (client.rs)

```rust
pub async fn place_order(
    &self,
    request: &TradeRequest,
) -> Result<TradeResult, Mt5Error>
```

Endpoint: `POST /order/send` — sama pola dengan `order_check` tapi ke `/order/send`.

---

## Public API (`lib.rs`)

### `EngineConfig`

Dikonfigurasi oleh `bin/hermes` per-run. `risk_pct` berlaku untuk semua symbol.

```rust
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub timeframe:    domain::Timeframe, // timeframe candle, e.g. M15
    pub candle_count: u32,               // jumlah candle di-fetch, e.g. 200
    pub risk_pct:     rust_decimal::Decimal, // e.g. 0.01 = 1% per trade
}
```

### `EngineOutcome`

Selalu di-return oleh `run_once`. Caller log dan lanjut ke siklus berikutnya.

```rust
#[derive(Debug)]
pub enum EngineOutcome {
    Traded {
        symbol: String,
        action: agents::Action,
        volume: rust_decimal::Decimal,
        order:  u64,
    },
    NoSignal,    // ICT tidak menghasilkan sinyal pada semua symbol
    Hold,        // semua kandidat ICT di-vote Hold oleh agents
    NoApproval,  // risk::evaluate menolak semua kandidat Buy/Sell
}
```

### `EngineError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("mt5 error: {0}")]
    Mt5(#[from] mt5_client::Mt5Error),

    #[error("agents error: {0}")]
    Agents(#[from] agents::AgentsError),

    #[error("order rejected: retcode={retcode}, comment={comment}")]
    OrderRejected { retcode: u32, comment: String },
}
```

### `run_once`

```rust
pub async fn run_once(
    symbols: &[&str],
    mt5:     &mt5_client::Mt5Client,
    llm:     &openrouter::OpenRouterClient,
    model:   &str,
    config:  &EngineConfig,
) -> Result<EngineOutcome, EngineError>
```

Synchronous dari sudut pandang caller — satu call, satu siklus, satu hasil.

---

## Pipeline Logic (`run_once`)

### Tahap 1 — Fetch context (parallel)

```
account, positions ← mt5.account() + mt5.positions()  (tokio::try_join!)
per symbol ← mt5.symbol(s) + mt5.rates_from_pos(s, tf, 0, count)  (JoinSet)
```

Jika fetch gagal untuk satu symbol → log warning, skip symbol itu, lanjut.

### Tahap 2 — Analyze per symbol (JoinSet, parallel)

Untuk setiap symbol yang berhasil di-fetch:

1. `IctAnalyzer::new(&candles).analyze()` → `IctAnalysis`
2. Jika `analysis.signal.is_none()` → skip (catat sebagai `no_signal`)
3. `agents::run_agents(llm, model, technical_in, sentiment_in, fundamental_in, risk_in)`
   - `FundamentalInput.news_context = ""`
   - `agents::RiskInput.signal = Some(&signal)`
4. Jika `decision.action == Action::Hold` → skip (catat sebagai `hold`)
5. Build `RiskConfig` dari `Symbol`:
   ```
   risk_pct      = config.risk_pct
   value_per_lot = symbol.trade_contract_size
   min_volume    = symbol.volume_min
   max_volume    = symbol.volume_max
   volume_step   = symbol.volume_step
   ```
6. `risk::evaluate(&account, &signal, &risk_config)` → `RiskDecision`
7. Jika `!risk_decision.approved` → skip (catat sebagai `no_approval`)
8. Simpan `Candidate { symbol, signal, decision, volume: risk_decision.volume }`

### Tahap 3 — Pilih winner

Dari semua `Candidate`, pilih yang `decision.confidence` tertinggi.

Jika tidak ada kandidat:
- Semua skip karena `no_signal` → return `EngineOutcome::NoSignal`
- Ada yang sampai agents tapi semua Hold → return `EngineOutcome::Hold`
- Ada yang sampai risk tapi semua rejected → return `EngineOutcome::NoApproval`

### Tahap 4 — Execute

```
request ← build_trade_request(&winner)
mt5.order_check(&request)    ← pre-flight (log hasilnya, tidak abort)
result  ← mt5.place_order(&request)
```

Jika `result.retcode != 10009` → return `Err(EngineError::OrderRejected { ... })`.

Return `EngineOutcome::Traded { symbol, action, volume, order: result.order }`.

---

## `execute.rs`

### `build_trade_request`

```rust
pub(crate) fn build_trade_request(
    symbol: &str,
    signal: &ict::TradeSignal,
    volume: rust_decimal::Decimal,
) -> mt5_client::TradeRequest
```

Mapping:
- `action = 1` (TRADE_ACTION_DEAL)
- `order_type`: `Side::Long → 0` (ORDER_TYPE_BUY), `Side::Short → 1` (ORDER_TYPE_SELL)
- `price = signal.entry` (Decimal → f64 di boundary)
- `sl = Some(signal.sl)`, `tp = Some(signal.tp)`
- `volume` (Decimal → f64 di boundary)
- `magic = None`, `comment = None`

---

## Testing

Semua test offline — tidak butuh MT5 atau LLM.

| Test | File | Assertion |
|------|------|-----------|
| `build_trade_request_buy` | `execute.rs` | `Side::Long` → `order_type=0`, price/sl/tp/volume correct |
| `build_trade_request_sell` | `execute.rs` | `Side::Short` → `order_type=1` |
| `risk_config_from_symbol` | `lib.rs` | `Symbol` fields → `RiskConfig` fields benar |
| `no_signal_returns_no_signal` | `lib.rs` | `IctAnalysis` tanpa signal → outcome `NoSignal` |

---

## Success Criteria

- `cargo test -p engine -p mt5-client` → semua test hijau
- `cargo clippy -p engine -p mt5-client -- -D warnings` → zero warnings
- `run_once` adalah single-shot: tidak ada loop internal, tidak ada state
- Multi-symbol diproses paralel (JoinSet)
- Satu trade per siklus — winner dengan confidence tertinggi
- `place_order` hanya dipanggil setelah `order_check` sukses
