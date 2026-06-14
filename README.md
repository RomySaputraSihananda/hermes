# Hermes — ICT Algorithmic Trading System

Hermes is a Rust-based algorithmic trading system implementing the **Inner Circle Trader (ICT)** methodology.
It provides a walk-forward backtester and a live MT5 trading engine.

---

## Architecture

```
hermes/
├── bin/
│   ├── backtest/        # Walk-forward backtester with PnL reporting
│   └── hermes/          # Live trading engine
└── crates/
    ├── domain/          # Core types: Candle, Side, Symbol, AccountInfo
    ├── ict/             # ICT signal detection: BOS/CHoCH, OTE, FVG, OB, PD arrays
    ├── risk/            # Fixed-fractional position sizing
    ├── mt5-client/      # MT5 bridge HTTP client
    └── engine/          # Orchestrates ICT analysis → risk → order execution
```

---

## ICT Strategy

Hermes analyzes a rolling window of M15 candles using a three-path signal cascade:

**OTE (Primary) → FVG (Fallback A) → OB (Fallback B)**

All paths require BOS/CHoCH confirmation + EMA trend alignment (H1 + optionally H4).

| Concept | Description |
|---------|-------------|
| **BOS / CHoCH** | Break of Structure / Change of Character — confirms directional bias |
| **OTE** | Optimal Trade Entry — 61.8–78.6% Fibonacci retracement of last impulse swing |
| **FVG** | Fair Value Gap — last close inside the most recent unmitigated imbalance zone |
| **Order Block** | Last opposite candle before an impulse; entry when price re-enters OB range |
| **PD Array** | Premium/Discount zones — SL at range_low (Long) or range_high (Short) |
| **Liquidity Sweep** | Tracked as optional confluence flag (no longer required for entry) |

---

## Quick Start

```bash
# 1. Configure environment
cp .env.example .env
# edit .env: set MT5_BASE_URL and SYMBOL

# 2. Run backtest
cargo run --release --bin backtest

# 3. Run live (set SYMBOLS= in .env)
cargo run --release --bin hermes
```

---

## Backtest Results (XAUUSDm, M15, 50k bars, $5,000 start, 1% risk)

| Metric | Value |
|--------|-------|
| Trades | 148 |
| Win Rate | 54.1% |
| **Profit Factor** | **3.90** |
| **Net Return** | **+209%** |
| Max Drawdown | -$476 |
| Final Balance | $15,433 |

Config: `MIN_RR=2.5`, `DOW_FILTER=true`, `TREND_H4_CONFIRM=true`, `PARTIAL_TP_1R=true`, `PARTIAL_TP_2R=true`, `TRAILING_2R=false`

---

## Configuration

See [`.env.example`](.env.example) for all options with comments.

Key variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `MT5_BASE_URL` | required | MT5 bridge URL |
| `SYMBOL` / `SYMBOLS` | required | Single symbol (backtest) / comma-separated (live) |
| `TIMEFRAME` | required | `M15` recommended |
| `CANDLE_COUNT` | required | Rolling window size (200 for M15) |
| `RISK_PCT` | `0.01` | Risk per trade (1%) |
| `MIN_RR` | `2.5` | Minimum reward:risk ratio |
| `KILLZONE_WINDOWS` | `08:00-13:00,15:00-18:00` | NY + London sessions |
| `DOW_FILTER` | `true` | Skip Monday and Friday |
| `TREND_H4_CONFIRM` | `true` | Require H4 EMA direction agreement |
| `PARTIAL_TP_1R` | `true` | Close 50% at 1R |
| `PARTIAL_TP_2R` | `true` | Close 25% of original at 2R |
| `FRIDAY_CLOSE_HOUR` | `21` | Force-close on Friday at this UTC hour |

---

## Requirements

- **Rust** 1.80+ (edition 2024)
- **MT5 bridge** running and accessible at `MT5_BASE_URL`

---

## CI/CD

Push a semver tag to trigger a GitHub Actions build and release:

```bash
git tag v1.0.0 && git push origin v1.0.0
```

Artifacts: `hermes-linux-x86_64`, `backtest-linux-x86_64`, `hermes-windows-x86_64.exe`, `backtest-windows-x86_64.exe`
