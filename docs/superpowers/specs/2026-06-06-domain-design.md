# Domain Crate — Design Spec

**Date:** 2026-06-06
**Phase:** 1
**Status:** Approved

---

## Tujuan

Implementasikan `crates/domain` — crate dasar yang mendefinisikan semua tipe data shared di seluruh workspace: `Candle`, `Tick`, `Symbol`, `AccountInfo`, `Position`, `Side`, dan `Timeframe`. Semua tipe di-derive `Deserialize` agar bisa di-parse langsung dari response JSON MT5 bridge (`http://192.168.1.105:8000`).

---

## Konteks API

MT5 bridge adalah FastAPI server. Response payload selalu:
```json
{ "data": [...], "count": N, "format": "json" }
```
Item dalam `data` adalah raw dict dari Python `mt5` library.

Key observations dari OpenAPI spec dan live endpoints:
- Timestamp dikembalikan sebagai naive string UTC: `"2026-06-05T19:05:00"` (tanpa `Z`)
- Timestamp millisecond: `"2026-06-05T00:00:00.234000"` (tanpa `Z`)
- Harga dikembalikan sebagai JSON number (`f64`): `59374.08`
- Position `type` field: integer `0` = BUY, `1` = SELL
- Timeframe query param: `"TIMEFRAME_M5"` (bukan `"M5"`)

---

## File Structure

```
crates/domain/src/
├── lib.rs              — pub mod + pub use semua tipe
├── timeframe.rs        — Timeframe enum + as_api_str()
├── candle.rs           — Candle struct
├── tick.rs             — Tick struct
├── symbol.rs           — Symbol struct
├── account.rs          — AccountInfo struct
├── position.rs         — Position struct + Side enum
└── serde_helpers.rs    — private helpers: de_decimal, naive_utc_*, de_side
```

---

## Types

### `Timeframe`

Semua 21 timeframe yang didukung MT5. Serialize/Deserialize ke format `"TIMEFRAME_M5"`.

| Variant | API string       | MT5 int |
|---------|-----------------|---------|
| M1      | TIMEFRAME_M1    | 1       |
| M2      | TIMEFRAME_M2    | 2       |
| M3      | TIMEFRAME_M3    | 3       |
| M4      | TIMEFRAME_M4    | 4       |
| M5      | TIMEFRAME_M5    | 5       |
| M6      | TIMEFRAME_M6    | 6       |
| M10     | TIMEFRAME_M10   | 10      |
| M12     | TIMEFRAME_M12   | 12      |
| M15     | TIMEFRAME_M15   | 15      |
| M20     | TIMEFRAME_M20   | 20      |
| M30     | TIMEFRAME_M30   | 30      |
| H1      | TIMEFRAME_H1    | 16385   |
| H2      | TIMEFRAME_H2    | 16386   |
| H3      | TIMEFRAME_H3    | 16387   |
| H4      | TIMEFRAME_H4    | 16388   |
| H6      | TIMEFRAME_H6    | 16390   |
| H8      | TIMEFRAME_H8    | 16392   |
| H12     | TIMEFRAME_H12   | 16396   |
| D1      | TIMEFRAME_D1    | 16408   |
| W1      | TIMEFRAME_W1    | 32769   |
| Mn1     | TIMEFRAME_MN1   | 49153   |

Method:
```rust
pub fn as_api_str(self) -> &'static str
```

Derive: `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize`

---

### `Side`

```rust
pub enum Side { Long, Short }
```

Serialize: `"long"` / `"short"` (lowercase). Dipakai di `Position` dan nanti di risk/agents.

Derive: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`

---

### `Candle`

Dari endpoint `/rates/from`, `/rates/from-pos`, `/rates/range`.

| Field       | Rust type      | Source          |
|-------------|---------------|-----------------|
| time        | DateTime<Utc>  | naive UTC string |
| open        | Decimal        | f64 from API    |
| high        | Decimal        | f64 from API    |
| low         | Decimal        | f64 from API    |
| close       | Decimal        | f64 from API    |
| tick_volume | u64            | integer         |
| spread      | i32            | integer         |
| real_volume | u64            | integer         |

Derive: `Debug, Clone, Deserialize`

---

### `Tick`

Dari endpoint `/ticks/from`, `/ticks/range`.

| Field       | Rust type      | Source               |
|-------------|---------------|----------------------|
| time        | DateTime<Utc>  | naive UTC secs       |
| time_msc    | DateTime<Utc>  | naive UTC millisecs  |
| bid         | Decimal        | f64 from API         |
| ask         | Decimal        | f64 from API         |
| last        | Decimal        | f64 from API (0 for crypto) |
| volume      | u64            | integer              |
| flags       | u32            | integer              |
| volume_real | Decimal        | f64 from API         |

Derive: `Debug, Clone, Deserialize`

---

### `Symbol`

Dari endpoint `/symbols/{symbol}`. Hanya subset yang relevan untuk trading — serde mengabaikan 60+ field sisanya secara otomatis.

| Field               | Rust type | Source         |
|--------------------|-----------|----------------|
| name               | String    |                |
| description        | String    |                |
| digits             | u8        | jumlah desimal |
| point              | Decimal   | f64 from API   |
| bid                | Decimal   | f64 from API   |
| ask                | Decimal   | f64 from API   |
| spread             | i32       |                |
| spread_float       | bool      |                |
| volume_min         | Decimal   | f64 from API   |
| volume_max         | Decimal   | f64 from API   |
| volume_step        | Decimal   | f64 from API   |
| trade_contract_size| Decimal   | f64 from API   |
| currency_base      | String    |                |
| currency_profit    | String    |                |
| category           | String    |                |

Derive: `Debug, Clone, Deserialize`

---

### `AccountInfo`

Dari endpoint `/account`. Data di-wrap dalam `DataResponse.data[0]`.

| Field          | Rust type | Source       |
|----------------|-----------|--------------|
| login          | u64       |              |
| leverage       | u32       |              |
| trade_allowed  | bool      |              |
| trade_expert   | bool      |              |
| currency       | String    |              |
| currency_digits| u8        |              |
| server         | String    |              |
| name           | String    |              |
| company        | String    |              |
| balance        | Decimal   | f64 from API |
| equity         | Decimal   | f64 from API |
| profit         | Decimal   | f64 from API |
| credit         | Decimal   | f64 from API |
| margin         | Decimal   | f64 from API |
| margin_free    | Decimal   | f64 from API |
| margin_level   | Decimal   | f64 from API |

Derive: `Debug, Clone, Deserialize`

---

### `Position`

Dari endpoint `/positions`. Field `type` di MT5 di-rename ke `side`.

| Field         | Rust type      | Notes                          |
|---------------|---------------|--------------------------------|
| ticket        | u64           |                                |
| symbol        | String        |                                |
| side          | Side          | `#[serde(rename = "type")]`, de_side: 0→Long, 1→Short |
| volume        | Decimal       | lots, f64 from API             |
| price_open    | Decimal       | f64 from API                   |
| sl            | Decimal       | stop loss, f64 from API        |
| tp            | Decimal       | take profit, f64 from API      |
| price_current | Decimal       | f64 from API                   |
| swap          | Decimal       | f64 from API                   |
| profit        | Decimal       | f64 from API                   |
| comment       | String        |                                |
| magic         | u64           |                                |

Derive: `Debug, Clone, Deserialize`

---

## Serialization Strategy

### f64 → Decimal (`de_decimal`)

```rust
// di serde_helpers.rs (private)
pub(crate) fn de_decimal<'de, D: Deserializer<'de>>(de: D) -> Result<Decimal, D::Error> {
    let f = f64::deserialize(de)?;
    Decimal::try_from(f).map_err(serde::de::Error::custom)
}
```

Dipakai via `#[serde(deserialize_with = "crate::serde_helpers::de_decimal")]`.

### Naive UTC string → DateTime<Utc>

Dua format yang dikembalikan API:
- Second precision: `"2026-06-05T19:05:00"` — format `%Y-%m-%dT%H:%M:%S`
- Millisecond precision: `"2026-06-05T00:00:00.234000"` — format `%Y-%m-%dT%H:%M:%S%.f`

```rust
// di serde_helpers.rs
pub(crate) mod naive_utc_secs { /* serialize + deserialize */ }
pub(crate) mod naive_utc_ms   { /* serialize + deserialize */ }
```

Dipakai via `#[serde(with = "crate::serde_helpers::naive_utc_secs")]`.

### Integer → Side (`de_side`)

```rust
pub(crate) fn de_side<'de, D: Deserializer<'de>>(de: D) -> Result<Side, D::Error> {
    match u8::deserialize(de)? {
        0 => Ok(Side::Long),
        1 => Ok(Side::Short),
        n => Err(serde::de::Error::custom(format!("unknown position type: {n}"))),
    }
}
```

### Zero external deps baru

Semua helper menggunakan `serde`, `chrono`, `rust_decimal` yang sudah ada di `domain/Cargo.toml`.

---

## Testing

Unit test inline di setiap file menggunakan JSON literal dari data real API.

| File          | Test                                      |
|---------------|-------------------------------------------|
| timeframe.rs  | `as_api_str()` round-trip, serde round-trip |
| candle.rs     | deserialize dari JSON real API            |
| tick.rs       | deserialize dari JSON real API (incl. time_msc) |
| symbol.rs     | deserialize dari JSON real API, unknown fields diabaikan |
| account.rs    | deserialize dari JSON real API            |
| position.rs   | deserialize JSON dengan `"type": 0` → Side::Long |

---

## Success Criteria

- `cargo test -p domain` → semua test hijau
- `cargo clippy -p domain -- -D warnings` → zero warnings
- Semua tipe bisa dipakai: `use domain::{Candle, Tick, Symbol, AccountInfo, Position, Side, Timeframe}`
- Tidak ada `f64` untuk price/money fields di public API crate
