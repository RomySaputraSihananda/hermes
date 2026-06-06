# ICT Min SL Distance Filter — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `min_sl_distance: Decimal` filter to `IctAnalyzer` so signals with SL too close to entry are discarded, wired via optional env var `MIN_SL_DISTANCE` into both the live engine and backtest binary.

**Architecture:** `IctAnalyzer::new` gains a second param `min_sl_distance: Decimal`; after `check_confluence` returns a signal, `analyze()` applies a filter that drops the signal when `|entry - sl| < min_sl_distance` (skipped when `min_sl_distance == 0`). Both `bin/hermes` and `bin/backtest` read the optional env var and forward the parsed value; `EngineConfig` grows a matching field.

**Tech Stack:** Rust, `rust_decimal::Decimal`, workspace crates: `ict`, `engine`, `bin/hermes`, `bin/backtest`.

**Spec:** `docs/superpowers/specs/2026-06-07-ict-min-sl-filter-design.md`

---

## File Map

| File | Action | Tanggung Jawab |
|------|--------|----------------|
| `crates/ict/src/analyzer.rs` | Modify | Struct field + constructor + filter + update 5 tests + 2 test baru |
| `crates/engine/src/lib.rs` | Modify | Tambah `min_sl_distance` ke `EngineConfig`; pass ke `IctAnalyzer::new` |
| `bin/hermes/src/main.rs` | Modify | Baca `MIN_SL_DISTANCE` env (opsional), tambah ke `EngineConfig` |
| `bin/backtest/src/main.rs` | Modify | Baca `MIN_SL_DISTANCE` env (opsional), pass ke `IctAnalyzer::new` |

---

## Task 1: IctAnalyzer — filter implementation (TDD)

**Files:**
- Modify: `crates/ict/src/analyzer.rs`

Background: `IctAnalyzer` saat ini hanya menyimpan `candles`. Constructor adalah `new(candles: &'a [Candle])`. Tests memanggil `IctAnalyzer::new(&candles).analyze()`. `full_confluence_candles()` menghasilkan sinyal dengan `entry=1.35`, `sl=1.25` → `|entry - sl| = 0.10`.

- [ ] **Step 1: Tulis 2 test baru yang akan gagal**

Tambah dua test di bawah `no_ob_or_fvg_in_ote_returns_no_signal` di blok `#[cfg(test)]`:

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

- [ ] **Step 2: Jalankan untuk memverifikasi gagal compile**

```bash
cargo test -p ict 2>&1 | head -20
```

Expected: compile error — `new` hanya menerima 1 argumen.

- [ ] **Step 3: Implementasi struct + constructor + filter**

Ubah `IctAnalyzer` struct dan `impl` block. **Ganti seluruh blok `pub struct IctAnalyzer` dan `impl<'a> IctAnalyzer<'a>`** (baris 8–69 di file saat ini) dengan:

```rust
pub struct IctAnalyzer<'a> {
    candles:         &'a [Candle],
    min_sl_distance: Decimal,
}

impl<'a> IctAnalyzer<'a> {
    pub fn new(candles: &'a [Candle], min_sl_distance: Decimal) -> Self {
        Self { candles, min_sl_distance }
    }

    pub fn analyze(&self) -> IctAnalysis {
        if self.candles.len() < 5 {
            return IctAnalysis {
                fvgs: vec![],
                order_blocks: vec![],
                structure: vec![],
                sweeps: vec![],
                pd_array: None,
                ote: None,
                signal: None,
            };
        }

        let swings       = detect_swings(self.candles, 2);
        let fvgs         = detect_fvg(self.candles);
        let order_blocks = detect_ob(self.candles, &swings);
        let structure    = detect_structure(self.candles, &swings);
        let sweeps       = detect_sweeps(self.candles, &swings);
        let pd_array     = compute_pd(self.candles, &swings);
        let bias         = structure.last().map(|s| s.side);
        let ote          = bias.and_then(|b| compute_ote(&swings, b));
        let signal       = check_confluence(
            self.candles,
            &fvgs,
            &order_blocks,
            &structure,
            &sweeps,
            &pd_array,
            &ote,
            &swings,
        );

        let signal = signal.filter(|s| {
            self.min_sl_distance == Decimal::ZERO
                || (s.entry - s.sl).abs() >= self.min_sl_distance
        });

        IctAnalysis {
            fvgs,
            order_blocks,
            structure,
            sweeps,
            pd_array,
            ote,
            signal,
        }
    }
}
```

- [ ] **Step 4: Jalankan tests — ekspektasi compile error pada test lama**

```bash
cargo test -p ict 2>&1 | head -30
```

Expected: compile error pada 5 test lama yang masih memanggil `IctAnalyzer::new(&candles)` tanpa argumen kedua.

- [ ] **Step 5: Update 5 test call sites yang ada**

Ganti setiap `IctAnalyzer::new(&candles).analyze()` dalam blok `#[cfg(test)]` menjadi `IctAnalyzer::new(&candles, Decimal::ZERO).analyze()`. Ada 5 lokasi:

1. Test `full_confluence_generates_signal`:
   ```rust
   let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
   ```

2. Test `no_bos_returns_no_signal`:
   ```rust
   let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
   ```

3. Test `wrong_pd_zone_returns_no_signal`:
   ```rust
   let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
   ```

4. Test `no_structure_in_ote_returns_no_signal`:
   ```rust
   let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
   ```

5. Test `no_ob_or_fvg_in_ote_returns_no_signal`:
   ```rust
   let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
   ```

- [ ] **Step 6: Jalankan semua tests dan pastikan pass**

```bash
cargo test -p ict 2>&1
```

Expected:
```
running 7 tests
test tests::full_confluence_generates_signal ... ok
test tests::no_bos_returns_no_signal ... ok
test tests::wrong_pd_zone_returns_no_signal ... ok
test tests::no_structure_in_ote_returns_no_signal ... ok
test tests::no_ob_or_fvg_in_ote_returns_no_signal ... ok
test tests::sl_too_close_filtered_out ... ok
test tests::sl_wide_enough_passes ... ok

test result: ok. 7 passed; 0 failed
```

- [ ] **Step 7: Commit**

```bash
git add crates/ict/src/analyzer.rs
git commit -m "feat(ict): add min_sl_distance filter to IctAnalyzer"
```

---

## Task 2: Wire up engine + hermes + backtest

**Files:**
- Modify: `crates/engine/src/lib.rs`
- Modify: `bin/hermes/src/main.rs`
- Modify: `bin/backtest/src/main.rs`

Background: Setelah Task 1, `IctAnalyzer::new` membutuhkan 2 argumen. Ada 2 call site produksi yang perlu diupdate:
- `crates/engine/src/lib.rs:93` — `ict::IctAnalyzer::new(&candles).analyze()`
- `bin/backtest/src/main.rs` — `IctAnalyzer::new(window).analyze()`

`EngineConfig` saat ini: `{ timeframe, candle_count, risk_pct }`. `bin/hermes/src/main.rs` membacanya dari env. Tidak ada unit test untuk binary files.

- [ ] **Step 1: Tambah `min_sl_distance` ke `EngineConfig` dan update call site di engine**

Di `crates/engine/src/lib.rs`, ganti struct `EngineConfig`:

```rust
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub timeframe:       domain::Timeframe,
    pub candle_count:    u32,
    pub risk_pct:        Decimal,
    pub min_sl_distance: Decimal,
}
```

Masih di `crates/engine/src/lib.rs`, ganti baris 93:

```rust
let analysis = ict::IctAnalyzer::new(&candles, config.min_sl_distance).analyze();
```

- [ ] **Step 2: Build engine untuk cek compile error downstream**

```bash
cargo build -p engine 2>&1 | head -20
```

Expected: compile error di `bin/hermes/src/main.rs` karena `EngineConfig { timeframe, candle_count, risk_pct }` kekurangan field `min_sl_distance`.

- [ ] **Step 3: Update `bin/hermes/src/main.rs`**

Tambah parsing `MIN_SL_DISTANCE` setelah parsing `risk_pct` (sebelum `cycle_secs`):

```rust
let min_sl_distance = std::env::var("MIN_SL_DISTANCE")
    .unwrap_or_else(|_| "0".to_string())
    .parse::<Decimal>()
    .context("MIN_SL_DISTANCE must be a decimal (e.g. 0.0010)")?;
```

Ganti konstruksi `EngineConfig`:

```rust
let config = EngineConfig { timeframe, candle_count, risk_pct, min_sl_distance };
```

- [ ] **Step 4: Update `bin/backtest/src/main.rs`**

Tambah parsing `MIN_SL_DISTANCE` setelah parsing `backtest_balance`:

```rust
let min_sl_distance = std::env::var("MIN_SL_DISTANCE")
    .unwrap_or_else(|_| "0".to_string())
    .parse::<Decimal>()
    .context("MIN_SL_DISTANCE must be a decimal (e.g. 0.0010)")?;
```

Cari baris `IctAnalyzer::new(window).analyze()` dan ganti dengan:

```rust
let analysis = IctAnalyzer::new(window, min_sl_distance).analyze();
```

- [ ] **Step 5: Build full workspace**

```bash
cargo build 2>&1 | tail -5
```

Expected:
```
Compiling engine v0.1.0 (...)
Compiling hermes v0.1.0 (...)
Compiling backtest v0.1.0 (...)
Finished `dev` profile ...
```

Zero errors.

- [ ] **Step 6: Jalankan seluruh test suite**

```bash
cargo test 2>&1 | tail -15
```

Expected: semua tests pass, termasuk 7 tests di `ict`.

- [ ] **Step 7: Clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src/lib.rs bin/hermes/src/main.rs bin/backtest/src/main.rs
git commit -m "feat(engine,hermes,backtest): wire MIN_SL_DISTANCE env var to IctAnalyzer"
```

---

## Success Criteria

- `cargo test -p ict` → 7 tests pass (5 lama + 2 baru)
- `cargo build` → zero errors
- `cargo clippy -- -D warnings` → zero warnings
- `MIN_SL_DISTANCE` tidak di-set → perilaku identik dengan sebelumnya
- `MIN_SL_DISTANCE=0.0010` → sinyal dengan `|entry - sl| < 0.0010` dibuang
- `MIN_SL_DISTANCE=invalid` → pesan error jelas + exit non-zero
