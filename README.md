# Hermes — ICT Algorithmic Trading System

Hermes is a Rust-based algorithmic trading system implementing the **Inner Circle Trader (ICT)** methodology. It provides a walk-forward backtester, a live trading engine, and optional LLM-powered trade confirmation agents.

---

## Architecture

```
hermes/
├── bin/
│   ├── backtest/        # Walk-forward backtester (main development tool)
│   └── hermes/          # Live trading engine
└── crates/
    ├── domain/          # Core types: Candle, Side, Symbol, AccountInfo
    ├── ict/             # ICT signal detection: BOS/CHoCH, OTE, OB, FVG, PD arrays
    ├── risk/            # Kelly-based position sizing
    ├── agents/          # LLM agents: technical, sentiment, fundamental, risk
    ├── openrouter/      # OpenRouter API client
    ├── mt5-client/      # MT5 bridge HTTP client
    └── engine/          # Live order execution engine
```

---

## ICT Strategy

Each H1 candle is analyzed over a rolling window using these ICT concepts:

| Concept | Description |
|---------|-------------|
| **BOS / CHoCH** | Break of Structure / Change of Character — confirms directional bias |
| **OTE** | Optimal Trade Entry — Fibonacci 61.8–78.6% retracement of last impulse swing |
| **PD Array** | Premium/Discount zones — entry only in discount (Long) or premium (Short) |
| **Order Block** | Last bearish candle before bullish impulse (Long OB) or vice versa |
| **FVG** | Fair Value Gap — imbalance in price delivery |
| **Liquidity Sweep** | Engineered liquidity run above/below swing highs/lows |

Signal requires confluence of: BOS/CHoCH + OTE zone + PD zone. OB and FVG are additional filters.

**SL placement**: at the high/low of the PD array range that contains the OTE zone.  
**TP placement**: at the most recent swing high (Long) or swing low (Short) outside the OTE.

---

## Backtest

### Quick Start

```bash
# Copy and configure .env
cp .env.example .env

# Run single pair
SYMBOL=GBPUSDm DATE_FROM=2025-01-01 DATE_TO=2025-12-31 cargo run --bin backtest

# Run with LLM agents disabled (faster)
USE_LLM=false SYMBOL=EURUSDm cargo run --bin backtest
```

### Environment Variables

#### Required
| Variable | Description |
|----------|-------------|
| `MT5_BASE_URL` | Base URL of the MT5 bridge (e.g. `http://localhost:8080`) |
| `SYMBOL` | Symbol name as registered in MT5 (e.g. `GBPUSDm`) |
| `TIMEFRAME` | Candle timeframe: `H1`, `H4`, `D1`, etc. |
| `CANDLE_COUNT` | Rolling window size in candles (ICT analysis window) |

#### Risk & Sizing
| Variable | Default | Description |
|----------|---------|-------------|
| `RISK_PCT` | `0.01` | Risk per trade as decimal (0.025 = 2.5%) |
| `BACKTEST_BALANCE` | `600` | Starting account balance in USD |
| `BACKTEST_CANDLES` | `50000` | Total historical candles to fetch |

#### Friction
| Variable | Default | Description |
|----------|---------|-------------|
| `COMMISSION_PER_LOT` | `0` | Round-trip commission in USD (e.g. `7.0` for Exness Zero) |
| `SLIPPAGE_POINTS` | `5` | Extra SL slippage in MT5 points (1 pip = 10 pts on 5-decimal pairs) |
| `SPREAD_OVERRIDE` | *(symbol default)* | Override spread in price units (set `0` for Zero/Raw accounts) |

#### Signal Filters
| Variable | Default | Description |
|----------|---------|-------------|
| `SWING_PERIOD` | `2` | Swing detection sensitivity — candle beats N neighbours on each side |
| `MIN_RR` | `0` | Minimum reward:risk ratio (e.g. `2.0` rejects trades below 2:1 R:R) |
| `MIN_SL_DISTANCE` | `0` | Fixed minimum SL distance in price units |
| `ATR_SL_MULT` | *(disabled)* | ATR(14) multiplier for dynamic minimum SL (e.g. `1.0`) |
| `KILLZONE_WINDOWS` | *(all hours)* | Comma-separated HH:MM-HH:MM windows in broker server time |

#### Trend Filter (EMA)
| Variable | Default | Description |
|----------|---------|-------------|
| `TREND_EMA_PERIOD` | `0` | EMA period for trend alignment filter. `0` = disabled |
| `TREND_TF` | `H4` | Timeframe for EMA: `H1` (uses existing candles) or `H4` (fetches separately) |

#### Trade Management (New)
| Variable | Default | Description |
|----------|---------|-------------|
| `BREAKEVEN_SL_1R` | `false` | After price reaches 1:1 R:R, move SL to entry (breakeven) |
| `PARTIAL_TP_1R` | `false` | Close 50% of position when price reaches 1:1 R:R, let remainder run |
| `TIMEOUT_CANDLES` | `0` | Close trade after N candles if still open. `0` = disabled |

#### Date Range & LLM
| Variable | Default | Description |
|----------|---------|-------------|
| `DATE_FROM` | *(all data)* | Signal filter start date: `YYYY-MM-DD` |
| `DATE_TO` | *(all data)* | Signal filter end date: `YYYY-MM-DD` |
| `USE_LLM` | `true` | Enable LLM trade confirmation agents |
| `OPENROUTER_API_KEY` | *(required if USE_LLM=true)* | OpenRouter API key |
| `LLM_MODEL` | *(required if USE_LLM=true)* | Model ID (e.g. `openai/gpt-4o-mini`) |

---

## Validated Pairs & Optimal Config

Backtested on H1 OHLC data, 2.5% risk per trade, Exness Zero account (commission $7/lot, spread ≈ 0).

| Pair | CC | MIN_RR | EMA | Notes |
|------|----|--------|-----|-------|
| `EURUSDm` | 100 | 2.0 | H1-20 | Tight window captures recent impulse |
| `GBPUSDm` | 400 | 5.0 | H1-20 | Wide window, high R:R filter for large moves |
| `NZDUSDm` | 100 | 2.0 | H1-20 | Follows EUR structure |
| `USDCADm` | 200 | 2.0 | H1-20 | CAD commodity-driven; recent bias matters |
| `USDJPYm` | 200 | 2.0 | H1-100 | BOJ policy creates longer trends; wider EMA |

Example run for GBP:
```bash
SYMBOL=GBPUSDm TIMEFRAME=H1 CANDLE_COUNT=400 MIN_RR=5.0 \
SWING_PERIOD=2 RISK_PCT=0.025 BACKTEST_BALANCE=600 \
TREND_EMA_PERIOD=20 TREND_TF=H1 USE_LLM=false \
COMMISSION_PER_LOT=7 SLIPPAGE_POINTS=5 \
DATE_FROM=2025-01-01 DATE_TO=2025-12-31 \
cargo run --bin backtest
```

---

## Portfolio Backtest Results (2025)

Portfolio of all 5 pairs, $600 account, 2.5% risk per pair, H1, `USE_LLM=false`.  
All 12 months of 2025 were profitable at the portfolio level.

### Income Strategy (monthly reset — withdraw profit, keep $600)

| Month | Portfolio P&L |
|-------|--------------|
| Jan   | +$485 |
| Feb   | +$518 |
| Mar   | +$133 |
| Apr   | +$216 |
| May   | +$756 |
| Jun   | +$1,882 |
| Jul   | +$1,147 |
| Aug   | +$478 |
| Sep   | +$380 |
| Oct   | +$873 |
| Nov   | +$1,134 |
| Dec   | +$810 |
| **Total** | **+$8,811** |
| **Avg/month** | **+$734** |

### Compound Strategy (`BREAKEVEN_SL_1R=true`)

Starting balance $600, risk 2.5% per pair compounds with the growing balance.

| Month | Balance |
|-------|---------|
| Jan   | $1,143 |
| Feb   | $2,320 |
| Mar   | $2,159 *(only losing month: -$161)* |
| Apr   | $3,787 |
| May   | $9,075 |
| Jun   | $25,378 |
| Jul   | $66,437 |
| Aug   | $130,365 |
| Sep   | $217,579 |
| Oct   | $270,598 |
| Nov   | $787,971 |
| Dec   | **$1,278,549** |

> **Note:** Compound returns are exceptional for 2025 — a year with unusually strong forex trends. Past performance does not guarantee future results. The aggressive compounding (2.5% × 5 pairs = 12.5% max concurrent risk) amplifies both gains and drawdown.

---

## Trade Management Features

### Breakeven SL (`BREAKEVEN_SL_1R=true`)

When price reaches the 1:1 R:R level (entry ± risk_distance), the stop loss is automatically moved to the original entry price. This turns the trade into a "free" position — worst case is breakeven minus slippage.

Effect on GBP 2025:
- Profit Factor: 11.73 → **22.74**
- Max Drawdown: -$833 → **-$337** (60% reduction)
- 25/30 "losses" were breakeven exits at avg -$19 instead of -$61

### Partial TP + Breakeven SL (`PARTIAL_TP_1R=true BREAKEVEN_SL_1R=true`)

At 1:1 R:R: close 50% of position and move SL to breakeven. The remaining 50% runs to the original TP.

Effect on GBP 2025:
- Win Rate: 22% → **86%**
- Max Consecutive Losses: 15 → **2**
- Max Drawdown: -$833 → **-$99**
- Trade log shows `PARTIAL1R` events and `BE_SL` exits

---

## Currency Conversion

For non-USD profit pairs (USDJPY, USDCHF, USDCAD), the system automatically adjusts:
- **Lot sizing**: `value_per_lot = contract_size / current_price` (ensures correct USD risk)
- **P&L conversion**: `profit_rate = 1 / exit_price` (converts native currency P&L to USD)

This is applied per-candle using the `currency_profit` field from MT5 symbol info.

---

## Requirements

- **Rust** 1.80+ (edition 2024)
- **MT5 bridge** running and accessible at `MT5_BASE_URL`
- **OpenRouter API key** (only if `USE_LLM=true`)

---

## .env Example

```env
MT5_BASE_URL=http://localhost:8080
OPENROUTER_API_KEY=sk-or-...
LLM_MODEL=openai/gpt-4o-mini

SYMBOL=GBPUSDm
TIMEFRAME=H1
CANDLE_COUNT=400
SWING_PERIOD=2
MIN_RR=5.0

RISK_PCT=0.025
BACKTEST_BALANCE=600
BACKTEST_CANDLES=50000

COMMISSION_PER_LOT=7
SLIPPAGE_POINTS=5
SPREAD_OVERRIDE=0

TREND_EMA_PERIOD=20
TREND_TF=H1
USE_LLM=false

# Optional trade management
BREAKEVEN_SL_1R=true
# PARTIAL_TP_1R=true
# TIMEOUT_CANDLES=48
```
