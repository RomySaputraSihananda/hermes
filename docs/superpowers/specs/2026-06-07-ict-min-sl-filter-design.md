# ICT Minimum SL Distance Filter — Design Spec

**Date:** 2026-06-07
**Phase:** 10
**Status:** Approved

---

## Tujuan

Tambahkan filter minimum SL distance ke `IctAnalyzer` agar sinyal dengan SL terlalu dekat dari entry dibuang sebelum masuk pipeline risk sizing. Hal ini mencegah risk engine menghitung volume yang tidak realistis (contoh: SL 0.1 pip → volume 77 lot).

---

## Env Var

| Var | Tipe | Contoh | Default | Deskripsi |
|-----|------|--------|---------|-----------|
| `MIN_SL_DISTANCE` | Decimal | `0.0010` | `0` (disabled) | Jarak minimum entry–SL dalam satuan harga. `0` = tidak ada filter. |

`MIN_SL_DISTANCE` bersifat opsional — jika tidak di-set, default `0` sehingga tidak ada perubahan perilaku.

Contoh nilai per symbol (broker 5-digit):
- EURUSD 10 pip = `0.0010`
- USDJPY 10 pip = `0.100`
- BTCUSD 10 pip = `1.00`

---

## Perubahan Architecture

### `crates/ict/src/analyzer.rs`

Tambah field `min_sl_distance: Decimal` ke struct `IctAnalyzer`:

```rust
pub struct IctAnalyzer<'a> {
    candles:         &'a [Candle],
    min_sl_distance: Decimal,
}
```

Constructor baru:

```rust
pub fn new(candles: &'a [Candle], min_sl_distance: Decimal) -> Self {
    Self { candles, min_sl_distance }
}
```

Filter di `analyze()` tepat setelah `check_confluence` mengembalikan signal:

```rust
let signal = signal.filter(|s| {
    self.min_sl_distance == Decimal::ZERO
        || (s.entry - s.sl).abs() >= self.min_sl_distance
});
```

- `Decimal::ZERO` berarti disabled (no filtering) — backward compatible untuk tests.
- Filter dibuat di `analyze()`, bukan di dalam `check_confluence` — `check_confluence` tetap pure (hanya menentukan sinyal terbaik berdasarkan struktur ICT).

### `crates/engine/src/lib.rs`

Tambah field ke `EngineConfig`:

```rust
pub struct EngineConfig {
    pub timeframe:       domain::Timeframe,
    pub candle_count:    u32,
    pub risk_pct:        Decimal,
    pub min_sl_distance: Decimal,  // baru
}
```

Call site diupdate:

```rust
// baris 93
ict::IctAnalyzer::new(&candles, config.min_sl_distance).analyze()
```

### `bin/hermes/src/main.rs`

Baca env var opsional (default `"0"`):

```rust
let min_sl_distance = std::env::var("MIN_SL_DISTANCE")
    .unwrap_or_else(|_| "0".to_string())
    .parse::<Decimal>()
    .context("MIN_SL_DISTANCE must be a decimal (e.g. 0.0010)")?;
```

Tambah ke `EngineConfig`:

```rust
let config = EngineConfig { timeframe, candle_count, risk_pct, min_sl_distance };
```

### `bin/backtest/src/main.rs`

Baca env var yang sama (opsional, default `"0"`):

```rust
let min_sl_distance = std::env::var("MIN_SL_DISTANCE")
    .unwrap_or_else(|_| "0".to_string())
    .parse::<Decimal>()
    .context("MIN_SL_DISTANCE must be a decimal (e.g. 0.0010)")?;
```

Pass langsung ke `IctAnalyzer`:

```rust
let analysis = IctAnalyzer::new(window, min_sl_distance).analyze();
```

---

## File Map

| File | Action | Tanggung Jawab |
|------|--------|----------------|
| `crates/ict/src/analyzer.rs` | Modify | Tambah field + update constructor + filter + update tests + 2 test baru |
| `crates/engine/src/lib.rs` | Modify | Tambah `min_sl_distance` ke `EngineConfig`, pass ke `IctAnalyzer::new` |
| `bin/hermes/src/main.rs` | Modify | Baca `MIN_SL_DISTANCE` env (opsional), tambah ke `EngineConfig` |
| `bin/backtest/src/main.rs` | Modify | Baca `MIN_SL_DISTANCE` env (opsional), pass ke `IctAnalyzer::new` |

---

## Testing

### Update test yang ada

Semua 5 call site `IctAnalyzer::new(&candles)` di `crates/ict/src/analyzer.rs` diupdate ke:

```rust
IctAnalyzer::new(&candles, Decimal::ZERO)
```

`Decimal::ZERO` = disabled → tidak ada perubahan perilaku pada test yang ada.

### 2 test baru di `crates/ict/src/analyzer.rs`

```rust
#[test]
fn sl_too_close_filtered_out() {
    // entry=1.35, sl=1.25 → distance=0.10
    // min_sl_distance=0.5 > 0.10 → signal dibuang
    let candles = full_confluence_candles();
    let analysis = IctAnalyzer::new(&candles, "0.5".parse().unwrap()).analyze();
    assert!(analysis.signal.is_none());
}

#[test]
fn sl_wide_enough_passes() {
    // entry=1.35, sl=1.25 → distance=0.10
    // min_sl_distance=0.05 < 0.10 → signal lolos
    let candles = full_confluence_candles();
    let analysis = IctAnalyzer::new(&candles, "0.05".parse().unwrap()).analyze();
    assert!(analysis.signal.is_some());
}
```

---

## Success Criteria

- `cargo test -p ict` → semua test pass termasuk 2 test baru
- `cargo build` (workspace) → zero errors
- `MIN_SL_DISTANCE` tidak di-set → perilaku sama seperti sebelumnya (disabled)
- `MIN_SL_DISTANCE=0.0010` → sinyal dengan SL < 10 pip dari entry dibuang
- `MIN_SL_DISTANCE=invalid` → error dengan pesan jelas + exit
