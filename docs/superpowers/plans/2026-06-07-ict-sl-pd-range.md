# ICT SL PD Range Placement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ganti SL placement di `check_confluence` dari swing low/high terdekat ke `pd.range_low` (Long) / `pd.range_high` (Short) — structural boundary keseluruhan price range.

**Architecture:** Satu perubahan di `check_confluence` di `crates/ict/src/analyzer.rs`. `pd` sudah tersedia di fungsi tersebut. Swing iterator 8 baris diganti 1 baris. `debug_assert!` Phase 11 diganti guard `if !valid_sl { return None; }`. Tidak ada perubahan di caller (engine, hermes, backtest).

**Tech Stack:** Rust, `rust_decimal::Decimal`, `crates/ict` workspace crate.

**Spec:** `docs/superpowers/specs/2026-06-07-ict-sl-pd-range-design.md`

---

## File Map

| File | Action | Tanggung Jawab |
|------|--------|----------------|
| `crates/ict/src/analyzer.rs` | Modify | Ganti swing iterator → pd.range_low/high; ganti debug_assert! → guard; tambah test fixture + test baru; update komentar 3 test |

---

## Task 1: SL PD range placement (TDD)

**Files:**
- Modify: `crates/ict/src/analyzer.rs`

**Background:**

File ini saat ini memiliki 8 test. Perubahan Phase 12:
1. Test `full_confluence_generates_signal` → ubah assertion message (nilai sl=1.0 tidak berubah)
2. Test `sl_too_close_filtered_out` → ubah komentar
3. Test `sl_wide_enough_passes` → ubah komentar + assertion message
4. Tambah fixture baru `two_swing_lows_candles()` dan test baru `sl_uses_pd_range_low_not_nearest_swing` (TDD — ini yang GAGAL lebih dulu)
5. Implementasi: ganti swing iterator → pd.range_low/high + guard

**Nilai penting:**
- Fixture `full_confluence_candles()`: `pd.range_low = 1.0` (sama dengan Phase 11 sl=1.0) → assertions tidak berubah
- Fixture baru `two_swing_lows_candles()`: dua swing low (1.00 dan 1.22), entry=1.35
  - Phase 11: sl = max(1.00, 1.22) = **1.22** (nearest)
  - Phase 12: sl = pd.range_low = **1.00** (structural)

---

- [ ] **Step 1: Baseline — pastikan semua test saat ini pass**

```bash
cargo test -p ict 2>&1
```

Expected:
```
test result: ok. 8 passed; 0 failed
```

Jika ada yang gagal, jangan lanjut — ada masalah di state awal.

---

- [ ] **Step 2: Tambah fixture baru `two_swing_lows_candles()`**

Di `crates/ict/src/analyzer.rs`, dalam blok `#[cfg(test)]`, setelah fungsi `full_confluence_candles()`, tambahkan:

```rust
fn two_swing_lows_candles() -> Vec<Candle> {
    // Designed to have two swing lows below entry=1.35:
    //   swing low A at 1.00 (candle[2])  → pd.range_low
    //   swing low B at 1.22 (candle[5])  → nearest to entry
    //
    // detect_swings(period=2) verification:
    //   [2].low=1.00: lows of [0]=1.40>[1]=1.25>1.00<[3]=1.30<[4]=1.28 ✓
    //   [5].low=1.22: lows of [3]=1.30>[4]=1.28>1.22<[6]=1.33<[7]=1.30 ✓
    //
    // ICT confluence: BOS Long at [11].close=2.10 > swing_high=2.00,
    //   OTE=[1.214,1.382], last_close=1.30 ∈ OTE,
    //   bullish OB [1]: top=1.45, bottom=1.25, overlaps OTE, not mitigated
    vec![
        make_candle("1.5",  "1.60", "1.40", "1.40"),  // [0]  low=1.40
        make_candle("1.40", "1.45", "1.25", "1.30"),  // [1]  bearish OB: top=1.45, bottom=1.25
        make_candle("1.30", "1.30", "1.00", "1.25"),  // [2]  swing_low=1.00 (pd.range_low)
        make_candle("1.25", "1.35", "1.30", "1.30"),  // [3]  low=1.30
        make_candle("1.30", "1.40", "1.28", "1.35"),  // [4]  low=1.28
        make_candle("1.35", "1.40", "1.22", "1.35"),  // [5]  swing_low=1.22 (nearest below 1.35)
        make_candle("1.35", "1.45", "1.33", "1.40"),  // [6]  low=1.33
        make_candle("1.40", "1.50", "1.30", "1.45"),  // [7]  low=1.30
        make_candle("1.45", "1.80", "1.45", "1.70"),  // [8]
        make_candle("1.70", "2.00", "1.70", "1.90"),  // [9]  swing_high=2.00
        make_candle("1.90", "1.95", "1.80", "1.90"),  // [10]
        make_candle("1.90", "1.95", "1.90", "2.10"),  // [11] BOS Long: close=2.10 > 2.00
        make_candle("2.10", "2.10", "0.95", "1.30"),  // [12] sweep + Discount + OTE close=1.30
    ]
}
```

---

- [ ] **Step 3: Tambah test baru yang akan GAGAL dengan implementasi sekarang**

Masih di blok `#[cfg(test)]`, tambahkan test baru di bawah `sl_is_on_correct_side_of_entry`:

```rust
#[test]
fn sl_uses_pd_range_low_not_nearest_swing() {
    // two_swing_lows_candles has two swing lows below entry=1.35:
    //   1.00 (pd.range_low) and 1.22 (nearest swing low)
    // Phase 12: sl must be pd.range_low=1.00, not nearest=1.22
    let candles = two_swing_lows_candles();
    let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
    let sig = analysis.signal.expect("two_swing_lows_candles should produce a signal");
    assert_eq!(
        sig.sl,
        "1.0".parse::<Decimal>().unwrap(),
        "sl should be pd.range_low (1.0), not nearest swing low (1.22)"
    );
}
```

---

- [ ] **Step 4: Jalankan tests — ekspektasi FAIL pada test baru**

```bash
cargo test -p ict sl_uses_pd_range_low_not_nearest_swing 2>&1
```

Expected output:
```
test tests::sl_uses_pd_range_low_not_nearest_swing ... FAILED

failures:
    tests::sl_uses_pd_range_low_not_nearest_swing

thread '...' panicked at 'assertion `left == right` failed: sl should be pd.range_low (1.0), not nearest swing low (1.22)
  left: 1.22
 right: 1.0'
```

Jika test PASS (bukan FAIL), itu berarti fixture tidak membedakan kedua implementasi — debug fixture sebelum lanjut.

Jika test gagal dengan `expect()` panic ("should produce a signal"), fixture tidak memenuhi ICT confluence — debug fixture sebelum lanjut.

---

- [ ] **Step 5: Implementasi — ganti swing iterator dengan pd.range_low/high + guard**

Di `crates/ict/src/analyzer.rs`, cari blok ini (baris ~129–147):

```rust
    let sl = match bias {
        Side::Long => swings
            .iter()
            .filter(|s| s.kind == SwingKind::Low && s.price < entry)
            .map(|s| s.price)
            .max()?,
        Side::Short => swings
            .iter()
            .filter(|s| s.kind == SwingKind::High && s.price > entry)
            .map(|s| s.price)
            .min()?,
    };

    // The filter predicates (price < entry for Long, price > entry for Short) already
    // guarantee this invariant by construction; the assert documents it for readers.
    debug_assert!(
        match bias { Side::Long => sl < entry, Side::Short => sl > entry },
        "SL must be on the protective side of entry"
    );
```

Ganti seluruh blok di atas dengan:

```rust
    let sl = match bias {
        Side::Long  => pd.range_low,
        Side::Short => pd.range_high,
    };
    let valid_sl = match bias {
        Side::Long  => sl < entry,
        Side::Short => sl > entry,
    };
    if !valid_sl {
        return None;
    }
```

---

- [ ] **Step 6: Update komentar di 3 test yang ada**

Di test `full_confluence_generates_signal`, ubah assertion message SL:

Dari:
```rust
assert_eq!(sig.sl, "1.0".parse::<rust_decimal::Decimal>().unwrap(), "sl should be nearest swing low below entry");
```

Menjadi:
```rust
assert_eq!(sig.sl, "1.0".parse::<rust_decimal::Decimal>().unwrap(), "sl should be pd.range_low");
```

Di test `sl_too_close_filtered_out`, ubah komentar:

Dari:
```rust
// entry=1.35, sl=1.0 → distance=0.35
// min_sl_distance=0.5 > 0.35 → signal dibuang
```

Menjadi:
```rust
// entry=1.35, sl=pd.range_low=1.0 → distance=0.35
// min_sl_distance=0.5 > 0.35 → signal dibuang
```

Di test `sl_wide_enough_passes`, ubah komentar:

Dari:
```rust
// entry=1.35, sl=1.0 → distance=0.35
// min_sl_distance=0.05 < 0.35 → signal lolos
```

Menjadi:
```rust
// entry=1.35, sl=pd.range_low=1.0 → distance=0.35
// min_sl_distance=0.05 < 0.35 → signal lolos
```

---

- [ ] **Step 7: Jalankan semua tests — ekspektasi semua pass**

```bash
cargo test -p ict 2>&1
```

Expected:
```
running 9 tests
test tests::full_confluence_generates_signal ... ok
test tests::no_bos_returns_no_signal ... ok
test tests::wrong_pd_zone_returns_no_signal ... ok
test tests::no_structure_in_ote_returns_no_signal ... ok
test tests::no_ob_or_fvg_in_ote_returns_no_signal ... ok
test tests::sl_too_close_filtered_out ... ok
test tests::sl_wide_enough_passes ... ok
test tests::sl_is_on_correct_side_of_entry ... ok
test tests::sl_uses_pd_range_low_not_nearest_swing ... ok

test result: ok. 9 passed; 0 failed
```

---

- [ ] **Step 8: Build full workspace**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished` tanpa error. Engine, hermes, backtest tidak berubah.

---

- [ ] **Step 9: Clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -5
```

Expected: zero warnings.

---

- [ ] **Step 10: Commit**

```bash
git add crates/ict/src/analyzer.rs
git commit -m "feat(ict): use pd.range_low/high as SL instead of nearest swing low/high"
```

---

## Success Criteria

- `cargo test -p ict` → 9 tests pass (8 lama + 1 baru)
- `cargo build` → zero errors
- `cargo clippy -- -D warnings` → zero warnings
- Test `sl_uses_pd_range_low_not_nearest_swing` membuktikan sl=1.0 (pd.range_low), bukan 1.22 (nearest swing)
- `debug_assert!` Phase 11 dihapus, diganti guard eksplisit
- Tidak ada perubahan di engine, hermes, backtest
