# ICT SL Swing Placement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ganti SL placement di `IctAnalyzer` dari tepi OB/FVG ke swing low/high structural terdekat, menghasilkan SL distance yang realistis (~10–30 pip) dan position sizing yang wajar.

**Architecture:** Satu perubahan di `check_confluence` di `crates/ict/src/analyzer.rs` — pisahkan komputasi entry (tetap dari OB/FVG midpoint) dari komputasi SL (baru dari swings). `?` pada swing lookup membuat signal tidak terbentuk jika tidak ada swing reference yang valid.

**Tech Stack:** Rust, `rust_decimal::Decimal`, `crates/ict` workspace crate.

**Spec:** `docs/superpowers/specs/2026-06-07-ict-sl-swing-placement-design.md`

---

## File Map

| File | Action | Tanggung Jawab |
|------|--------|----------------|
| `crates/ict/src/analyzer.rs` | Modify | Pisahkan SL dari OB/FVG block; hitung SL dari swings; update 3 test assertions |

---

## Task 1: SL swing placement (TDD)

**Files:**
- Modify: `crates/ict/src/analyzer.rs`

**Background:** `check_confluence` saat ini mengambil SL dari `ob.bottom`/`fvg.bottom` (Long) atau `ob.top`/`fvg.top` (Short). Ini hanya 0.2–1 pip dari entry. Setelah perubahan, SL akan diambil dari swing low tertinggi di bawah entry (Long) atau swing high terendah di atas entry (Short) — data `swings` sudah tersedia di `check_confluence`.

Test fixture `full_confluence_candles()` menghasilkan:
- Entry = OB midpoint = (1.45 + 1.25) / 2 = 1.35
- Satu-satunya swing low di bawah entry = candle [2] dengan low = 1.0
- New SL = 1.0 (bukan lagi 1.25)
- New sl distance = |1.35 - 1.0| = 0.35

- [ ] **Step 1: Update test assertions ke nilai baru**

Di `crates/ict/src/analyzer.rs`, cari dan update test `full_confluence_generates_signal`:

Ubah baris ini:
```rust
assert_eq!(sig.sl, "1.25".parse::<rust_decimal::Decimal>().unwrap(), "sl should be OB bottom");
```

Menjadi:
```rust
assert_eq!(sig.sl, "1.0".parse::<rust_decimal::Decimal>().unwrap(), "sl should be nearest swing low below entry");
```

Cari dan update test `sl_too_close_filtered_out` — hanya komentar:
```rust
#[test]
fn sl_too_close_filtered_out() {
    // entry=1.35, sl=1.0 → distance=0.35
    // min_sl_distance=0.5 > 0.35 → signal dibuang
    let candles = full_confluence_candles();
    let analysis = IctAnalyzer::new(&candles, "0.5".parse().unwrap()).analyze();
    assert!(analysis.signal.is_none());
}
```

Cari dan update test `sl_wide_enough_passes` — komentar + assertion:
```rust
#[test]
fn sl_wide_enough_passes() {
    // entry=1.35, sl=1.0 → distance=0.35
    // min_sl_distance=0.05 < 0.35 → signal lolos
    let candles = full_confluence_candles();
    let analysis = IctAnalyzer::new(&candles, "0.05".parse().unwrap()).analyze();
    assert!(analysis.signal.is_some());
    let sig = analysis.signal.unwrap();
    assert_eq!(sig.side, Side::Long);
    assert_eq!(sig.entry, "1.35".parse::<Decimal>().unwrap());
    assert_eq!(sig.sl, "1.0".parse::<Decimal>().unwrap());
}
```

- [ ] **Step 2: Jalankan tests — ekspektasi FAIL**

```bash
cargo test -p ict 2>&1 | grep -E "FAILED|test result"
```

Expected: `full_confluence_generates_signal` dan `sl_wide_enough_passes` gagal karena implementasi masih menggunakan `sl = ob.bottom = 1.25` bukan `1.0`.

- [ ] **Step 3: Implementasi — ganti SL logic di `check_confluence`**

Di `crates/ict/src/analyzer.rs`, cari blok ini (ada di `check_confluence`, setelah Condition 4):

```rust
    let (entry_top, entry_bottom, sl) = if let Some(ob) = overlapping_ob {
        let sl = match bias {
            Side::Long  => ob.bottom,
            Side::Short => ob.top,
        };
        (ob.top, ob.bottom, sl)
    } else if let Some(fvg) = overlapping_fvg {
        let sl = match bias {
            Side::Long  => fvg.bottom,
            Side::Short => fvg.top,
        };
        (fvg.top, fvg.bottom, sl)
    } else {
        return None;
    };

    // Generate TradeSignal
    let two = Decimal::from(2u32);
    let entry = (entry_top + entry_bottom) / two;
```

Ganti dengan:

```rust
    let (entry_top, entry_bottom) = if let Some(ob) = overlapping_ob {
        (ob.top, ob.bottom)
    } else if let Some(fvg) = overlapping_fvg {
        (fvg.top, fvg.bottom)
    } else {
        return None;
    };

    let two   = Decimal::from(2u32);
    let entry = (entry_top + entry_bottom) / two;

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
```

Bagian selanjutnya (komputasi `tp`, `sweep_present`, `confluence`, `TradeSignal`) **tidak berubah** — biarkan seperti apa adanya.

- [ ] **Step 4: Jalankan semua tests dan pastikan pass**

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

- [ ] **Step 5: Build full workspace**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished` tanpa error. Tidak ada perubahan di files lain — caller `engine` dan `backtest` tidak perlu diubah karena signature `IctAnalyzer::new` tidak berubah.

- [ ] **Step 6: Clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -5
```

Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/ict/src/analyzer.rs
git commit -m "feat(ict): place SL at nearest swing low/high instead of OB/FVG edge"
```

---

## Success Criteria

- `cargo test -p ict` → 7 tests pass
- `cargo build` → zero errors
- `cargo clippy -- -D warnings` → zero warnings
- `sig.sl` di `full_confluence_generates_signal` = `1.0` (swing low), bukan `1.25` (OB bottom)
- Tidak ada perubahan di engine, hermes, backtest — hanya `crates/ict/src/analyzer.rs`
