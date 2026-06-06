# risk Crate — Design Spec

**Date:** 2026-06-06
**Phase:** 6
**Status:** Approved

---

## Tujuan

Implementasikan `crates/risk` — pure-computation risk management layer yang menghitung position sizing (fixed % risk) dan memvalidasi kelayakan trade sebelum dikirim ke MT5.

Digunakan oleh Phase 7 (`engine` crate) sebagai firewall terakhir sebelum eksekusi order.

---

## Dependency

```toml
[dependencies]
domain        = { workspace = true }   # AccountInfo, TradeSignal (via ict)
ict           = { workspace = true }   # TradeSignal
rust_decimal  = { workspace = true }   # Decimal arithmetic
thiserror     = { workspace = true }   # (reserved, tidak dipakai di fase ini)
```

---

## File Structure

```
crates/risk/src/
├── lib.rs    — pub use + pub fn evaluate()
└── types.rs  — RiskConfig, RiskDecision
```

---

## Types (`types.rs`)

### `RiskConfig`

Dikonfigurasi oleh `engine` per-symbol. `value_per_lot` dihitung engine dari symbol info MT5 (contract size × tick value / tick size).

```rust
#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub risk_pct: rust_decimal::Decimal,      // 0.01 = 1% dari balance
    pub value_per_lot: rust_decimal::Decimal, // account currency per 1.0 lot per 1.0 price unit
    pub min_volume: rust_decimal::Decimal,    // lot minimum broker, e.g. 0.01
    pub max_volume: rust_decimal::Decimal,    // lot maksimum broker, e.g. 10.0
    pub volume_step: rust_decimal::Decimal,   // lot step broker, e.g. 0.01
}
```

### `RiskDecision`

Selalu di-return — caller cek `.approved`. Volume adalah `0` jika ditolak.

```rust
#[derive(Debug, Clone)]
pub struct RiskDecision {
    pub volume: rust_decimal::Decimal,  // lot yang dihitung (0 jika !approved)
    pub approved: bool,
    pub reason: Option<String>,         // alasan penolakan jika !approved
}
```

---

## Entry Point (`lib.rs`)

```rust
pub fn evaluate(
    account: &domain::AccountInfo,
    signal: &ict::TradeSignal,
    config: &RiskConfig,
) -> RiskDecision
```

Synchronous, pure computation — tidak ada I/O, tidak ada async.

---

## Logic `evaluate()`

```
1. sl_distance = |signal.entry − signal.sl|
   → jika sl_distance == 0:
       return RiskDecision { volume: 0, approved: false, reason: "SL equals entry" }

2. risk_amount = account.balance × config.risk_pct

3. raw_volume = risk_amount / (sl_distance × config.value_per_lot)

4. volume = floor(raw_volume / config.volume_step) × config.volume_step
   (bulatkan ke bawah ke nearest volume_step)

5. jika volume < config.min_volume:
   return RiskDecision { volume: 0, approved: false, reason: "volume below minimum" }

6. volume = min(volume, config.max_volume)
   (clamp ke atas tanpa reject)

7. jika account.margin_free <= 0:
   return RiskDecision { volume: 0, approved: false, reason: "insufficient margin" }

8. return RiskDecision { volume, approved: true, reason: None }
```

### Contoh Nyata

| Parameter | Value |
|-----------|-------|
| balance | $5,000 |
| risk_pct | 0.01 (1%) |
| entry | 1.1000 |
| sl | 1.0950 |
| sl_distance | 0.0050 |
| value_per_lot | 10,000 (EURUSD standard lot) |
| risk_amount | $50 |
| raw_volume | 50 / (0.005 × 10000) = **1.00** |
| volume_step | 0.01 |
| **final volume** | **1.00 lot** |

---

## Testing

Inline `#[cfg(test)]` di `lib.rs`. Semua test offline — tidak butuh MT5 atau koneksi.

| Test | Setup | Assertion |
|------|-------|-----------|
| `calculates_volume_correctly` | balance=5000, risk_pct=0.01, entry=1.1000, sl=1.0950, value_per_lot=10000, step=0.01 | volume=1.00, approved=true |
| `volume_floored_to_step` | raw_volume menghasilkan 0.037, step=0.01 | volume=0.03 (floor, bukan round) |
| `rejects_when_sl_equals_entry` | entry == sl | approved=false, reason contains "SL" |
| `rejects_when_volume_below_minimum` | risk kecil → raw_volume=0.003, min_volume=0.01 | approved=false, reason contains "minimum" |
| `rejects_when_margin_free_zero` | margin_free=0 | approved=false, reason contains "margin" |

---

## Success Criteria

- `cargo test -p risk` → 5 tests hijau
- `cargo clippy -p risk -- -D warnings` → zero warnings
- `use risk::{evaluate, RiskConfig, RiskDecision}` — semua accessible
- `evaluate` adalah pure function: tidak ada side effects, tidak ada I/O
- `volume` selalu di-floor ke `volume_step` (bukan round)
