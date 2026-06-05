# mt5-client Crate — Design Spec

**Date:** 2026-06-06
**Phase:** 2
**Status:** Approved

---

## Tujuan

Implementasikan `crates/mt5-client` — thin async HTTP client untuk berkomunikasi dengan MT5 bridge FastAPI (`http://192.168.1.105:8000`). Crate ini mengekspos 7 typed async methods yang mengembalikan domain types dari crate `domain`.

---

## Konteks API

MT5 bridge adalah FastAPI server di `http://192.168.1.105:8000`. Tidak ada auth. Semua timestamps adalah naive UTC string (tanpa `Z`). Hasil live check response format:

| Endpoint | Method | `data` format |
|----------|--------|--------------|
| `/health` | GET | flat object (tidak di-wrap) |
| `/account` | GET | `[account_obj]` array 1 item |
| `/symbols/{symbol}` | GET | `[symbol_obj]` array 1 item |
| `/symbols/{symbol}/tick` | GET | `[tick_obj]` array 1 item |
| `/rates/from-pos` | GET | `[candle, ...]` array N item |
| `/positions` | GET | `[position, ...]` array N item |
| `/order/check` | POST | single object (bukan array) |

Error body dari bridge:
```json
{"type": "/errors/...", "title": "...", "status": 503, "detail": "MT5 not connected"}
```

---

## File Structure

```
crates/mt5-client/src/
├── lib.rs      — pub use Mt5Client, Mt5Error, public types
├── error.rs    — Mt5Error enum
├── types.rs    — TradeRequest, OrderCheckResult, HealthStatus
│                 + private: DataVec<T>, DataOne<T>, ApiErrorBody
└── client.rs   — Mt5Client struct + impl semua 7 method
```

---

## Public API

### `Mt5Client`

```rust
pub struct Mt5Client {
    base_url: String,
    http: reqwest::Client,
}

impl Mt5Client {
    pub fn new(base_url: impl Into<String>) -> Self;
}
```

### `Mt5Error`

```rust
#[derive(Debug, thiserror::Error)]
pub enum Mt5Error {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("bridge error ({status}): {detail}")]
    Api { status: u16, detail: String },

    #[error("response parse failed: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("empty response for {endpoint}")]
    Empty { endpoint: &'static str },
}
```

### 7 Method Signatures

```rust
impl Mt5Client {
    pub async fn health(&self) -> Result<HealthStatus, Mt5Error>;
    pub async fn account(&self) -> Result<AccountInfo, Mt5Error>;
    pub async fn symbol(&self, name: &str) -> Result<Symbol, Mt5Error>;
    pub async fn tick(&self, symbol: &str) -> Result<Tick, Mt5Error>;
    pub async fn rates_from_pos(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_pos: u32,
        count: u32,
    ) -> Result<Vec<Candle>, Mt5Error>;
    pub async fn positions(&self) -> Result<Vec<Position>, Mt5Error>;
    pub async fn order_check(
        &self,
        request: &TradeRequest,
    ) -> Result<OrderCheckResult, Mt5Error>;
}
```

---

## Public Types (di `types.rs`)

### `HealthStatus`

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub mt5_connected: bool,
    pub mt5_version: String,
    pub api_version: String,
}
```

### `TradeRequest`

Dikirim sebagai POST body ke `/order/check` dalam wrapper `{"request": <TradeRequest>}`.

Bridge mengharapkan:
```json
{"request": {"action": 1, "symbol": "BTCUSDm", "volume": 0.01, "type": 0, "price": 59970.0}}
```

Implementasi: buat anonymous struct `struct Body<'a> { request: &'a TradeRequest }` lalu serialize ke JSON.

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct TradeRequest {
    pub action: u32,
    pub symbol: String,
    pub volume: f64,
    #[serde(rename = "type")]
    pub order_type: u32,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}
```

> `volume`, `price`, `sl`, `tp` menggunakan `f64` karena ini adalah request keluar ke broker — nilai sudah final dari risk engine, tidak di-aritmetika lebih lanjut.

### `OrderCheckResult`

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OrderCheckResult {
    pub retcode: u32,
    pub balance: f64,
    pub equity: f64,
    pub profit: f64,
    pub margin: f64,
    pub margin_free: f64,
    pub margin_level: f64,
    pub comment: String,
}
```

---

## Internal Types (private di `types.rs`)

```rust
#[derive(serde::Deserialize)]
struct DataVec<T> { data: Vec<T> }

#[derive(serde::Deserialize)]
struct DataOne<T> { data: T }

#[derive(serde::Deserialize)]
struct ApiErrorBody { status: u16, detail: String }
```

---

## Error Handling Flow

Setiap method:
1. Build URL, kirim request via `reqwest`
2. Network error → `Mt5Error::Http`
3. HTTP status ≥ 400 → parse `ApiErrorBody` → `Mt5Error::Api`
4. `.text()` → `serde_json::from_str` → gagal → `Mt5Error::Parse`
5. Array kosong saat expect 1 item → `Mt5Error::Empty`
6. Success → return unwrapped domain type

---

## Logging

`tracing::debug!(endpoint = %url, "mt5 response ok")` per successful call. Tidak log sensitive data (harga, volume, credentials).

---

## Testing

Offline JSON literal tests — tidak butuh server hidup. 7 test cases:

| Test | Assertion utama |
|------|----------------|
| `parse_health` | `mt5_connected == true` |
| `parse_account` | `login == 415817698` |
| `parse_symbol` | `name == "BTCUSDm"` |
| `parse_tick` | `bid > Decimal::ZERO` |
| `parse_rates` | `len == 5`, `candles[0].open > Decimal::ZERO` |
| `parse_positions_empty` | `len == 0` |
| `parse_order_check` | `retcode == 0` |

JSON literals di-capture dari live API sebelum dibuat test.

---

## Query Parameters (`rates_from_pos`)

```
GET /rates/from-pos?symbol=BTCUSDm&timeframe=TIMEFRAME_M5&start_pos=0&count=5
```

`timeframe` dikirim sebagai string via `Timeframe::as_api_str()`.

---

## Constraints

- **No f64 untuk domain types** — semua price/lot di `AccountInfo`, `Symbol`, `Tick`, `Candle`, `Position` tetap `Decimal` (dari `domain` crate)
- **No auth** — bridge tidak memerlukan auth header
- **Secrets** — tidak ada secrets di mt5-client; bridge di internal network
- **Paper mode default** — `order_check` hanya dipakai untuk validasi, bukan eksekusi order

---

## Success Criteria

- `cargo test -p mt5-client` → semua 7 test hijau
- `cargo clippy -p mt5-client -- -D warnings` → zero warnings
- `Mt5Client::new("http://...")` bisa di-construct dan di-pass ke crate lain
