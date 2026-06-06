# bin/hermes — Design Spec

**Date:** 2026-06-06
**Phase:** 8
**Status:** Approved

---

## Tujuan

Implementasikan `bin/hermes` — entry point utama trading bot hermes. Binary ini membaca konfigurasi dari environment, membuat semua client, lalu menjalankan trading loop yang memanggil `engine::run_once` secara berulang.

Tidak ada logic baru di binary ini — semua logic ada di crates masing-masing. Binary hanya menghubungkan (wiring) semua crates dan mengelola loop.

---

## Dependency

```toml
[dependencies]
engine             = { workspace = true }
domain             = { workspace = true }
anyhow             = { workspace = true }
dotenvy            = { workspace = true }
mt5-client         = { workspace = true }
openrouter         = { workspace = true }
rust_decimal       = { workspace = true }
tokio              = { workspace = true }
tracing            = { workspace = true }
tracing-subscriber = { workspace = true }
```

---

## File Structure

```
bin/hermes/src/
└── main.rs    — startup, config parsing, trading loop

crates/domain/src/
└── timeframe.rs  — tambah FromStr impl
```

---

## Domain Change: `Timeframe::from_str`

Tambah `FromStr` impl ke `domain::Timeframe` agar env var `TIMEFRAME=M15` bisa di-parse langsung:

```rust
impl std::str::FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "M1"  => Ok(Self::M1),
            "M2"  => Ok(Self::M2),
            "M3"  => Ok(Self::M3),
            "M4"  => Ok(Self::M4),
            "M5"  => Ok(Self::M5),
            "M6"  => Ok(Self::M6),
            "M10" => Ok(Self::M10),
            "M12" => Ok(Self::M12),
            "M15" => Ok(Self::M15),
            "M20" => Ok(Self::M20),
            "M30" => Ok(Self::M30),
            "H1"  => Ok(Self::H1),
            "H2"  => Ok(Self::H2),
            "H3"  => Ok(Self::H3),
            "H4"  => Ok(Self::H4),
            "H6"  => Ok(Self::H6),
            "H8"  => Ok(Self::H8),
            "H12" => Ok(Self::H12),
            "D1"  => Ok(Self::D1),
            "W1"  => Ok(Self::W1),
            "MN1" => Ok(Self::Mn1),
            other => Err(format!("unknown timeframe: {other}")),
        }
    }
}
```

Test: `"M15".parse::<Timeframe>()` → `Ok(Timeframe::M15)`, `"invalid".parse::<Timeframe>()` → `Err`.

---

## Environment Variables

| Var | Tipe | Contoh | Deskripsi |
|-----|------|--------|-----------|
| `MT5_BASE_URL` | String | `http://localhost:8000` | Base URL MT5 bridge |
| `OPENROUTER_API_KEY` | String | `sk-or-...` | Dibaca langsung oleh `OpenRouterClient` |
| `LLM_MODEL` | String | `anthropic/claude-sonnet-4-5` | Model untuk `agents::run_agents` |
| `SYMBOLS` | String | `EURUSDm,BTCUSDm` | Comma-separated symbol list |
| `TIMEFRAME` | String | `M15` | Timeframe untuk ICT analysis |
| `CANDLE_COUNT` | u32 | `200` | Jumlah candle yang di-fetch |
| `RISK_PCT` | Decimal | `0.01` | Persentase risk per trade (1% = 0.01) |
| `CYCLE_SECS` | u64 | `60` | Interval antar siklus dalam detik |
| `RUST_LOG` | String | `info` | Tracing log level (optional, default `info`) |

Semua var kecuali `RUST_LOG` dan `OPENROUTER_API_KEY` (dibaca oleh crate-nya sendiri) dibaca dan di-validate di startup. Binary exit dengan pesan jelas jika ada yang missing atau invalid.

---

## Startup Sequence (`main.rs`)

```
1. dotenvy::dotenv().ok()          — load .env file (silent jika tidak ada)
2. tracing_subscriber::fmt()        — setup logging dengan EnvFilter dari RUST_LOG
3. Baca env vars                    — semua var di tabel atas
4. Parse dan validate               — anyhow::bail! jika invalid
5. Mt5Client::new(mt5_base_url)
6. OpenRouterClient::new()          — baca OPENROUTER_API_KEY otomatis
7. EngineConfig { timeframe, candle_count, risk_pct }
8. symbols: Vec<String>             — split SYMBOLS by ","
9. Masuk trading loop
```

---

## Trading Loop

```rust
loop {
    let start = tokio::time::Instant::now();
    
    let symbol_refs: Vec<&str> = symbols.iter().map(String::as_str).collect();
    
    match engine::run_once(&symbol_refs, &mt5, &llm, &model, &config).await {
        Ok(EngineOutcome::Traded { symbol, action, volume, order }) => {
            tracing::info!(symbol, ?action, %volume, order, "trade executed");
        }
        Ok(EngineOutcome::NoSignal)   => tracing::debug!("no ICT signal on any symbol"),
        Ok(EngineOutcome::Hold)       => tracing::debug!("agents voted hold on all symbols"),
        Ok(EngineOutcome::NoApproval) => tracing::debug!("risk rejected all candidates"),
        Err(e) => tracing::error!(error = %e, "cycle error, continuing"),
    }
    
    let elapsed = start.elapsed();
    let cycle   = std::time::Duration::from_secs(cycle_secs);
    if elapsed < cycle {
        tokio::time::sleep(cycle - elapsed).await;
    }
}
```

Loop tidak pernah exit kecuali Ctrl+C (SIGINT) yang kill process secara langsung. Tidak ada graceful shutdown handler di Phase 8.

---

## Testing

Binary tidak punya unit test — tidak ada logic baru untuk di-test.

| Test | File | Assertion |
|------|------|-----------|
| `from_str_known_variants` | `domain/timeframe.rs` | `"M15".parse()` → `Ok(Timeframe::M15)` |
| `from_str_unknown_returns_err` | `domain/timeframe.rs` | `"invalid".parse::<Timeframe>()` → `Err` |

Build smoke test: `cargo build -p hermes` harus compile tanpa error.

---

## Success Criteria

- `cargo build -p hermes` → kompilasi sukses
- `cargo test -p domain` → test `from_str` hijau
- Binary startup: jika env var missing → pesan error jelas + exit code non-zero
- Binary startup: jika semua env var valid → log `info` muncul, loop berjalan
- Loop: setiap siklus log outcome (`traded` / `no signal` / `hold` / `no approval` / `error`)
- Loop: interval antar siklus ≈ `CYCLE_SECS` detik (dikompensasi waktu eksekusi)
