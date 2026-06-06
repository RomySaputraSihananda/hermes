# Backtest System — Design Spec

**Date:** 2026-06-06
**Phase:** 9
**Status:** Approved

---

## Tujuan

Implementasikan `bin/backtest` — binary standalone yang mereplay data historis dari MT5 bridge melalui pipeline ICT → agents → risk yang sama dengan live bot, lalu mensimulasikan eksekusi trade dan melaporkan per-trade log beserta summary statistik.

Tidak ada logic baru — semua crates yang sudah ada (`ict`, `agents`, `risk`) dipakai as-is. Yang baru hanya wiring walk-forward dan simulasi eksekusi.

---

## Workspace Addition

Tambah ke `Cargo.toml` workspace:
```toml
members = [
    ...
    "bin/backtest",
]
```

---

## Dependencies (`bin/backtest/Cargo.toml`)

```toml
[package]
name    = "backtest"
version = "0.1.0"
edition = "2024"

[dependencies]
agents       = { workspace = true }
anyhow       = { workspace = true }
domain       = { workspace = true }
dotenvy      = { workspace = true }
ict          = { workspace = true }
mt5-client   = { workspace = true }
openrouter   = { workspace = true }
risk         = { workspace = true }
rust_decimal = { workspace = true }
tokio        = { workspace = true }
tracing      = { workspace = true }
tracing-subscriber = { workspace = true }
```

---

## Environment Variables

| Var | Tipe | Contoh | Deskripsi |
|-----|------|--------|-----------|
| `MT5_BASE_URL` | String | `http://192.168.1.105:8000` | MT5 bridge URL |
| `OPENROUTER_API_KEY` | String | `sk-or-...` | Dibaca otomatis oleh OpenRouterClient |
| `LLM_MODEL` | String | `anthropic/claude-sonnet-4-5` | Model untuk agents |
| `SYMBOL` | String | `EURUSDm` | Satu symbol yang di-backtest |
| `TIMEFRAME` | String | `M15` | Timeframe candle |
| `CANDLE_COUNT` | u32 | `200` | Ukuran sliding window (sama seperti live) |
| `RISK_PCT` | Decimal | `0.01` | Persentase risk per trade |
| `BACKTEST_CANDLES` | usize | `2000` | Total candles historis yang di-fetch |
| `BACKTEST_BALANCE` | Decimal | `5000` | Initial balance simulasi |

---

## File Structure

```
bin/backtest/src/
└── main.rs   — startup, data fetch, walk-forward loop, simulation, output
```

---

## Startup Sequence

```
1. dotenvy::dotenv().ok()
2. tracing_subscriber setup
3. Baca + validasi semua env vars
4. Mt5Client::new(mt5_base_url)
5. OpenRouterClient::new()
6. Fetch data (satu kali):
   - mt5.symbol(SYMBOL)          → symbol_info
   - mt5.rates_from_pos(SYMBOL, timeframe, 0, BACKTEST_CANDLES) → candles (oldest→newest)
   - mt5.account()               → account_info (untuk RiskConfig awal)
7. Masuk walk-forward loop
```

Candles dari `rates_from_pos` sudah dalam urutan oldest→newest (sesuai test data di mt5-client).

---

## Walk-forward Loop

```
window_size = CANDLE_COUNT
total       = candles.len()
balance     = BACKTEST_BALANCE
open_trade  = None

for i in 0..(total - window_size):
    window  = &candles[i .. i + window_size]
    trigger = candles[i + window_size]   // candle tepat setelah window

    analysis = IctAnalyzer::new(window).analyze()
    if analysis.signal.is_none() → continue
    if open_trade.is_some() → continue  // satu trade sekaligus

    signal = analysis.signal.unwrap()

    technical_in   = TechnicalInput   { symbol, candles: window, analysis }
    sentiment_in   = SentimentInput   { symbol, candles: window }
    fundamental_in = FundamentalInput { symbol, news_context: "" }
    risk_in        = RiskInput        { account: &account_sim, positions: &[], signal: Some(&signal) }

    decision = agents::run_agents(llm, model, ...).await?
    if decision.action == Action::Hold → continue

    risk_cfg = RiskConfig {
        risk_pct:      RISK_PCT,
        value_per_lot: symbol_info.trade_contract_size,
        min_volume:    symbol_info.volume_min,
        max_volume:    symbol_info.volume_max,
        volume_step:   symbol_info.volume_step,
    }
    risk_dec = risk::evaluate(&account_sim, &signal, &risk_cfg)
    if !risk_dec.approved → continue

    open_trade = Some(SimTrade {
        open_time: trigger.time,
        signal,
        volume: risk_dec.volume,
        entry_index: i + window_size,
    })
```

`account_sim` adalah `domain::AccountInfo` buatan lokal dengan `balance` dan `margin_free = balance`.

---

## Trade Simulation

Setelah open trade dibuka, setiap candle berikutnya dicek:

```
for j in (entry_index + 1)..total:
    candle = candles[j]

    match signal.side:
        Long:
            sl_hit = candle.low  <= signal.sl
            tp_hit = candle.high >= signal.tp
        Short:
            sl_hit = candle.high >= signal.sl
            tp_hit = candle.low  <= signal.tp

    if sl_hit && tp_hit:
        exit = signal.sl  // worst case
        outcome = SL
    else if tp_hit:
        exit = signal.tp
        outcome = TP
    else if sl_hit:
        exit = signal.sl
        outcome = SL
    else:
        continue

    break
```

Jika TP/SL tidak kena sampai akhir candles → trade ditutup di harga close candle terakhir, outcome = TIMEOUT.

---

## P&L Calculation

```
Long:  pnl = (exit - entry) × volume × contract_size
Short: pnl = (entry - exit) × volume × contract_size
```

`balance += pnl` setelah trade ditutup.

Max drawdown dihitung sebagai penurunan terbesar dari peak balance:
```
peak = max(peak, balance)
drawdown = min(drawdown, balance - peak)
```

---

## Output Format

### Per-trade log (satu baris per trade, dicetak saat trade tutup):

```
[2024-01-15 09:00 M15] EURUSD LONG  entry=1.08500 tp=1.09000 sl=1.08200 vol=0.10 → TP   exit=1.09000 pnl=+500.00 bal=5500.00
[2024-01-15 14:00 M15] EURUSD SHORT entry=1.27000 tp=1.26400 sl=1.27300 vol=0.05 → SL   exit=1.27300 pnl=-150.00 bal=5350.00
[2024-01-16 08:15 M15] EURUSD LONG  entry=1.08300 tp=1.08800 sl=1.08050 vol=0.12 → TIMEOUT exit=1.08400 pnl=+120.00 bal=5470.00
```

### Summary di akhir:

```
─────────────────────────────────────────
Backtest: EURUSDm M15 | 2000 candles
─────────────────────────────────────────
Trades     : 15
Win        : 10  (66.7%)
Loss       : 4   (26.7%)
Timeout    : 1   (6.7%)
─────────────────────────────────────────
Total PnL  : +1470.00
Max Drawdown: -230.00
Final Balance: 6470.00
─────────────────────────────────────────
```

Output ke stdout dengan `println!` (bukan tracing) agar mudah di-pipe atau di-redirect ke file.

---

## Testing

Binary ini tidak punya unit test — semua logic tersebar di satu loop sederhana, dan seluruh computation logic sudah teruji di crates masing-masing.

Build smoke test: `cargo build -p backtest` harus compile.

---

## Success Criteria

- `cargo build -p backtest` → kompilasi sukses
- Startup dengan env var missing → pesan error jelas + exit
- Walk-forward loop berjalan tanpa panic
- Per-trade log muncul satu baris per trade tertutup
- Summary muncul di akhir dengan angka yang konsisten
- Balance update benar setelah setiap trade
