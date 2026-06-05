# ICT Strategy Primitives — Design Spec

**Date:** 2026-06-06
**Phase:** 3
**Status:** Approved

---

## Tujuan

Implementasikan `crates/ict` — crate yang mendeteksi 6 ICT (Inner Circle Trader) strategy primitives dari `Vec<Candle>` dan menghasilkan entry signal lengkap (entry, SL, TP) jika semua confluence terpenuhi.

---

## Dependency

```toml
[dependencies]
domain    = { workspace = true }   # Candle, Side, Decimal
thiserror = { workspace = true }
tracing   = { workspace = true }
```

Tidak ada dependency baru.

---

## File Structure

```
crates/ict/src/
├── lib.rs       — mod declarations + pub use semua public types
├── types.rs     — semua public data types
├── swing.rs     — SwingPoint detection (pub(crate))
├── detect.rs    — semua detection functions (pub(crate))
└── analyzer.rs  — IctAnalyzer, IctAnalysis, check_confluence (pub)
```

---

## Public Types (`types.rs`)

### `Fvg` (Fair Value Gap)

3-candle pattern:
- **Bullish FVG**: `candle[i-2].high < candle[i].low` → gap ke atas, bertindak sebagai support
- **Bearish FVG**: `candle[i-2].low > candle[i].high` → gap ke bawah, bertindak sebagai resistance

```rust
#[derive(Debug, Clone)]
pub struct Fvg {
    pub top: Decimal,
    pub bottom: Decimal,
    pub formed_at: DateTime<Utc>,   // waktu candle tengah (impulse)
    pub side: Side,                 // Long = bullish, Short = bearish
    pub mitigated: bool,
}
```

**Mitigated:**
- Bullish FVG: candle berikutnya dengan `low ≤ fvg.top` → mitigated
- Bearish FVG: candle berikutnya dengan `high ≥ fvg.bottom` → mitigated

---

### `OrderBlock`

Candle terakhir berlawanan arah sebelum swing:
- **Bullish OB**: candle bearish (`close < open`) terakhir sebelum swing low
- **Bearish OB**: candle bullish (`close > open`) terakhir sebelum swing high

```rust
#[derive(Debug, Clone)]
pub struct OrderBlock {
    pub top: Decimal,           // high candle OB
    pub bottom: Decimal,        // low candle OB
    pub formed_at: DateTime<Utc>,
    pub side: Side,
    pub mitigated: bool,
}
```

**Mitigated:**
- Bullish OB: candle berikutnya dengan `close < ob.bottom` → mitigated
- Bearish OB: candle berikutnya dengan `close > ob.top` → mitigated

---

### `BosChoch` (Break of Structure / Change of Character)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureEvent { Bos, Choch }

#[derive(Debug, Clone)]
pub struct BosChoch {
    pub kind: StructureEvent,
    pub level: Decimal,         // swing level yang di-break
    pub broken_at: DateTime<Utc>,
    pub side: Side,             // Long = break ke atas, Short = ke bawah
}
```

**Deteksi:**
- **BOS**: candle close melewati swing high/low terbaru dalam arah yang sama dengan trend
- **CHoCH**: candle close melewati swing dalam arah berlawanan (reversal — dalam uptrend, close di bawah Higher Low)

---

### `LiquiditySweep`

Wick melampaui swing high/low tetapi close kembali di dalam range.

```rust
#[derive(Debug, Clone)]
pub struct LiquiditySweep {
    pub level: Decimal,
    pub swept_at: DateTime<Utc>,
    pub side: Side,             // Long = swept lows, Short = swept highs
}
```

---

### `PdArray` (Premium / Discount)

Zona relatif terhadap swing range terbaru.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdZone { Premium, Discount, Equilibrium }

#[derive(Debug, Clone)]
pub struct PdArray {
    pub range_high: Decimal,
    pub range_low: Decimal,
    pub equilibrium: Decimal,   // (range_high + range_low) / 2
    pub current_zone: PdZone,   // berdasarkan close candle terakhir
}
```

- `close > equilibrium` → Premium
- `close < equilibrium` → Discount
- `close == equilibrium` → Equilibrium

---

### `Ote` (Optimal Trade Entry)

Fibonacci 61.8%–78.6% retracement dari swing terbaru.

```rust
#[derive(Debug, Clone)]
pub struct Ote {
    pub top: Decimal,
    pub bottom: Decimal,
    pub side: Side,
}
```

**Konstanta Fibonacci:**
- `FIB_618 = 0.618`
- `FIB_786 = 0.786`

**Long OTE** (dari swing_low → swing_high, menunggu pullback):
```
range  = swing_high - swing_low
top    = swing_high - FIB_618 * range
bottom = swing_high - FIB_786 * range
```

**Short OTE** (dari swing_high → swing_low, menunggu pullback ke atas):
```
range  = swing_high - swing_low
top    = swing_low + FIB_786 * range
bottom = swing_low + FIB_618 * range
```

---

### `TradeSignal`

```rust
#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub side: Side,
    pub entry: Decimal,       // midpoint OB/FVG yang overlap dengan OTE
    pub sl: Decimal,          // Long: ob.bottom; Short: ob.top
    pub tp: Decimal,          // Long: max swing high; Short: min swing low
    pub confluence: ConfluenceFlags,
}

#[derive(Debug, Clone, Default)]
pub struct ConfluenceFlags {
    pub has_bos_choch: bool,
    pub in_pd_zone: bool,
    pub ob_in_ote: bool,
    pub fvg_in_ote: bool,
    pub sweep_present: bool,   // ada sweep dalam 5 candle terakhir
}
```

---

## Internal Types (`swing.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SwingKind { High, Low }

#[derive(Debug, Clone)]
pub(crate) struct SwingPoint {
    pub(crate) index: usize,
    pub(crate) price: Decimal,
    pub(crate) kind: SwingKind,
    pub(crate) time: DateTime<Utc>,
}
```

**`detect_swings(candles: &[Candle], n: usize) -> Vec<SwingPoint>`**

```
for i in n..candles.len()-n:
    if candle[i].high > max(high dari [i-n..i] dan [i+1..=i+n]):
        tambah SwingHigh
    if candle[i].low < min(low dari [i-n..i] dan [i+1..=i+n]):
        tambah SwingLow
```

Butuh minimal `2*n+1` candle. Default `n=2` → minimum 5 candle.

---

## Detection Functions (`detect.rs`)

Semua fungsi `pub(crate)`, dipanggil dari `analyzer.rs`.

| Fungsi | Signature |
|--------|-----------|
| `detect_fvg` | `(candles: &[Candle]) -> Vec<Fvg>` |
| `detect_ob` | `(candles: &[Candle], swings: &[SwingPoint]) -> Vec<OrderBlock>` |
| `detect_structure` | `(candles: &[Candle], swings: &[SwingPoint]) -> Vec<BosChoch>` |
| `detect_sweeps` | `(candles: &[Candle], swings: &[SwingPoint]) -> Vec<LiquiditySweep>` |
| `compute_pd` | `(candles: &[Candle], swings: &[SwingPoint]) -> Option<PdArray>` |
| `compute_ote` | `(swings: &[SwingPoint], bias: Side) -> Option<Ote>` |

---

## Analyzer (`analyzer.rs`)

```rust
pub struct IctAnalyzer<'a> {
    candles: &'a [Candle],
}

impl<'a> IctAnalyzer<'a> {
    pub fn new(candles: &'a [Candle]) -> Self;
    pub fn analyze(&self) -> IctAnalysis;
}

pub struct IctAnalysis {
    pub fvgs: Vec<Fvg>,
    pub order_blocks: Vec<OrderBlock>,
    pub structure: Vec<BosChoch>,
    pub sweeps: Vec<LiquiditySweep>,
    pub pd_array: Option<PdArray>,
    pub ote: Option<Ote>,
    pub signal: Option<TradeSignal>,
}
```

**Urutan dalam `analyze()`:**
1. `detect_swings(candles, 2)` → `swings`
2. `detect_fvg(candles)` → `fvgs`
3. `detect_ob(candles, &swings)` → `order_blocks`
4. `detect_structure(candles, &swings)` → `structure`
5. `detect_sweeps(candles, &swings)` → `sweeps`
6. `compute_pd(candles, &swings)` → `pd_array`
7. `compute_ote(&swings, bias)` → `ote`  ← bias dari BOS/CHoCH terbaru (step 4)
8. `check_confluence(...)` → `signal`

Jika `candles.len() < 5`, return `IctAnalysis` dengan semua field kosong/None.

---

## Confluence Logic (`check_confluence`)

**4 kondisi wajib — semua harus terpenuhi:**

```
1. Ada BosChoch → ambil yang terbaru, tentukan bias (Side)

2. bias Long  → pd_array.current_zone == Discount
   bias Short → pd_array.current_zone == Premium
   Jika tidak match → None

3. Hitung OTE. Cek last_close ada di dalam OTE zone (ote.bottom ≤ close ≤ ote.top)
   Jika tidak → None

4. Cari struktur (OB atau FVG) yang:
   - Belum mitigated
   - Overlap dengan OTE: struktur.top > ote.bottom AND struktur.bottom < ote.top
   - Priority: OB lebih diutamakan dari FVG
   Jika tidak ada → None

5. Generate TradeSignal:
   - entry = (overlapping_structure.top + overlapping_structure.bottom) / 2
   - sl    = Long → overlapping_ob.bottom (atau fvg.bottom)
              Short → overlapping_ob.top (atau fvg.top)
   - tp    = Long  → max(swing.price untuk semua SwingHigh di swings)
              Short → min(swing.price untuk semua SwingLow di swings)
   - confluence.sweep_present = ada LiquiditySweep dalam 5 candle terakhir
```

---

## Testing

Inline `#[cfg(test)]` di setiap file. Semua test menggunakan **synthetic candles** via helper `make_candle(o, h, l, c)`.

| File | Tests |
|------|-------|
| `swing.rs` | `swing_high_detected`, `swing_low_detected`, `insufficient_candles_returns_empty` |
| `detect.rs` | `bullish_fvg`, `bearish_fvg`, `fvg_mitigated_after_touch`, `bullish_ob`, `bearish_ob`, `bos_bullish`, `choch_bearish`, `sweep_above_high`, `sweep_below_low`, `pd_discount`, `pd_premium`, `ote_long`, `ote_short` |
| `analyzer.rs` | `full_confluence_generates_signal`, `no_bos_returns_no_signal`, `wrong_pd_zone_returns_no_signal`, `no_structure_in_ote_returns_no_signal` |

**Total: 20 tests**

**Helper:**
```rust
fn make_candle(o: &str, h: &str, l: &str, c: &str) -> Candle {
    use chrono::DateTime;
    Candle {
        time: DateTime::default(),
        open: o.parse().unwrap(),
        high: h.parse().unwrap(),
        low:  l.parse().unwrap(),
        close: c.parse().unwrap(),
        tick_volume: 100,
        spread: 10,
        real_volume: 0,
    }
}
```

---

## Success Criteria

- `cargo test -p ict` → semua 20 tests hijau
- `cargo clippy -p ict -- -D warnings` → zero warnings
- `use ict::{IctAnalyzer, IctAnalysis, TradeSignal, Fvg, OrderBlock, BosChoch, LiquiditySweep, PdArray, Ote}` — semua accessible
- Tidak ada panic path — semua edge case (candles terlalu sedikit, tidak ada swing, dll) return `None` atau empty Vec
