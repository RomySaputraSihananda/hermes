# ICT SL PD Range Placement — Design Spec

**Date:** 2026-06-07
**Phase:** 12
**Status:** Approved

---

## Tujuan

Perbaiki SL placement di `IctAnalyzer` dari swing low/high terdekat (yang menghasilkan SL micro-structure 0.1–0.5 pip dari entry) ke batas struktural PD range (`pd.range_low` untuk Long, `pd.range_high` untuk Short).

**Root cause Phase 11:** `detect_swings(period=2)` pada M5 menghasilkan swing point setiap 2–3 candle. Swing low "terdekat di bawah entry" seringkali hanya 0.1–0.5 pip di bawah entry → SL distance ≈ 0 → position sizing tidak realistis (vol=104, vol=200 lot).

**Solusi:** `pd.range_low` / `pd.range_high` adalah batas ekstrem dari seluruh price range di window (dikomputasi dari swing high/low terlebar di window, bukan hanya terdekat ke entry). Ini merepresentasikan structural boundary yang tepat secara ICT — level di mana setup invalidated sepenuhnya.

---

## Perubahan Architecture

Satu-satunya file yang berubah: **`crates/ict/src/analyzer.rs`** — di dalam fungsi `check_confluence`.

### Sebelum (Phase 11)

SL diambil dari swing low/high terdekat ke entry:

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
debug_assert!(
    match bias { Side::Long => sl < entry, Side::Short => sl > entry },
    "SL must be on the protective side of entry"
);
```

### Sesudah (Phase 12)

SL diambil dari batas ekstrem PD range:

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

- `pd` sudah tersedia di baris sebelumnya: `let pd = pd_array.as_ref()?;`
- Guard `valid_sl` menggantikan `debug_assert!` — explicit return None, bukan crash di debug
- Tidak ada parameter baru, tidak ada struct baru, tidak ada perubahan di caller

---

## File Map

| File | Action | Tanggung Jawab |
|------|--------|----------------|
| `crates/ict/src/analyzer.rs` | Modify | Ganti swing iterator → `pd.range_low`/`pd.range_high`; ganti `debug_assert!` → guard; update komentar test |

---

## Testing

### Fixture behavior (tidak berubah)

Di `full_confluence_candles()`:
- Swing low terlebar = candle[2] low = `1.0` = `pd.range_low`
- `pd.range_high` tidak digunakan (Long fixture)

Nilai `sl == "1.0"` sama antara Phase 11 dan Phase 12 untuk fixture ini — **tidak ada assertion yang perlu diubah**.

### Update komentar di 3 tests

Hanya komentar source-of-SL yang diperbarui:

**`full_confluence_generates_signal`:**
```rust
assert_eq!(sig.sl, "1.0".parse::<rust_decimal::Decimal>().unwrap(), "sl should be pd.range_low");
```

**`sl_too_close_filtered_out`:**
```rust
// entry=1.35, sl=pd.range_low=1.0 → distance=0.35
// min_sl_distance=0.5 > 0.35 → signal dibuang
```

**`sl_wide_enough_passes`:**
```rust
// entry=1.35, sl=pd.range_low=1.0 → distance=0.35
// min_sl_distance=0.05 < 0.35 → signal lolos
```

### Test baru: guard valid_sl

```rust
#[test]
fn sl_at_pd_range_boundary_invalid_returns_no_signal() {
    // Konstruksi buatan di mana pd.range_low >= entry (edge case)
    // tidak perlu fixture lengkap — covered oleh condition return None
    // Test ini hanya memastikan guard aktif: jika range_low >= entry maka None
    // (lihat catatan di bawah — test ini opsional karena kondisi ini
    //  praktis tidak terjadi di data nyata dan sudah di-guard dengan return None)
}
```

Catatan: Test eksplisit untuk guard `valid_sl` **tidak wajib** karena kondisinya tidak terjadi di fixture yang ada dan di-guard oleh `return None` yang jelas. Cukup pastikan `debug_assert!` lama dihapus dan diganti guard.

---

## Success Criteria

- `cargo test -p ict` → 8 tests pass (tidak ada yang berubah, hanya komentar)
- `cargo build` (full workspace) → zero errors
- `cargo clippy -- -D warnings` → zero warnings
- Backtest dengan `MIN_SL_DISTANCE=0` menunjukkan SL distance realistis (10–80 pip, bukan 0.1–0.5 pip)
- Volume per trade wajar (0.01–0.1 lot pada balance $200)
- `debug_assert!` Phase 11 dihapus, diganti guard eksplisit `if !valid_sl { return None; }`
