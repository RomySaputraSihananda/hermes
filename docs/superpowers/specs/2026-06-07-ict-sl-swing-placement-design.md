# ICT SL Swing Placement — Design Spec

**Date:** 2026-06-07
**Phase:** 11
**Status:** Approved

---

## Tujuan

Perbaiki SL placement di `IctAnalyzer` dari "tepi OB/FVG" ke "swing low/high structural terdekat". Ini menyelesaikan akar masalah dari Phase 10: SL yang hanya 0.2–1 pip menghasilkan volume tidak realistis karena risk engine membagi risk_amount dengan jarak SL yang sangat kecil.

Dengan SL structural (swing low/high), jarak SL di M5 menjadi ~10–30 pip — menghasilkan position sizing yang wajar.

---

## Perubahan Architecture

Satu-satunya file yang berubah: **`crates/ict/src/analyzer.rs`** — di dalam fungsi `check_confluence`.

### Sebelum

SL diambil dari tepi OB/FVG bersamaan dengan `entry_top` dan `entry_bottom`:

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
```

### Sesudah

OB/FVG hanya menghasilkan `entry_top` dan `entry_bottom`. SL dihitung terpisah dari `swings` setelah `entry` diketahui:

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

- **Long**: SL = swing low tertinggi yang masih di bawah entry (protective level paling dekat di bawah entry)
- **Short**: SL = swing high terendah yang masih di atas entry (protective level paling dekat di atas entry)
- `?` pada `max()?` / `min()?`: jika tidak ada swing reference yang valid → return `None` (signal tidak terbentuk). Setup tanpa SL structural yang jelas tidak valid secara ICT.

---

## File Map

| File | Action | Tanggung Jawab |
|------|--------|----------------|
| `crates/ict/src/analyzer.rs` | Modify | Pisahkan SL dari OB/FVG block; hitung SL dari swings setelah entry diketahui; update tests |

---

## Testing

### Update test yang ada

`full_confluence_generates_signal` — ubah assertion SL:

```rust
// sebelum
assert_eq!(sig.sl, "1.25".parse::<Decimal>().unwrap(), "sl should be OB bottom");

// sesudah
assert_eq!(sig.sl, "1.0".parse::<Decimal>().unwrap(), "sl should be nearest swing low below entry");
```

Reasoning: entry = 1.35 (OB midpoint). Satu-satunya swing low di bawah 1.35 adalah candle [2] dengan low = 1.0. Swing low di candle [10] (last candle, low=0.95) tidak terdeteksi karena `detect_swings` membutuhkan N candle di kedua sisi.

### Update komentar di 2 filter tests

`sl_too_close_filtered_out`:
```rust
// sebelum: entry=1.35, sl=1.25 → distance=0.10
// sesudah: entry=1.35, sl=1.0  → distance=0.35
// min_sl_distance=0.5 > 0.35 → signal dibuang (masih valid)
```

`sl_wide_enough_passes`:
```rust
// sebelum: entry=1.35, sl=1.25 → distance=0.10
// sesudah: entry=1.35, sl=1.0  → distance=0.35
// min_sl_distance=0.05 < 0.35 → signal lolos (masih valid)

// update assertion:
assert_eq!(sig.sl, "1.0".parse::<Decimal>().unwrap());
```

---

## Success Criteria

- `cargo test -p ict` → semua 7 tests pass
- `cargo build` (full workspace) → zero errors
- Backtest dengan `MIN_SL_DISTANCE=0` menunjukkan SL distance yang realistis (bukan 0.2–1 pip)
- Volume per trade menjadi wajar (tidak ada 26-lot trade pada balance kecil)
