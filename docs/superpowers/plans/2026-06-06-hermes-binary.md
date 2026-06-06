# bin/hermes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire all hermes crates into a runnable trading bot binary that reads config from environment variables and runs `engine::run_once` in a timed loop.

**Architecture:** Two tasks — first add `FromStr` to `domain::Timeframe` (needed to parse the `TIMEFRAME` env var), then implement `bin/hermes/src/main.rs` which loads `.env`, validates all env vars, creates clients, and runs the loop. No new logic — just wiring.

**Tech Stack:** Rust async (tokio), `dotenvy`, `tracing-subscriber`, `anyhow`, all workspace crates.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/domain/src/timeframe.rs` | Modify | Add `FromStr` impl + 2 tests |
| `bin/hermes/Cargo.toml` | Modify | Add `domain`, `mt5-client`, `openrouter`, `rust_decimal`, `tracing` deps |
| `bin/hermes/src/main.rs` | Modify | Startup, env parsing, loop |

---

### Task 1: domain::Timeframe — FromStr

**Files:**
- Modify: `crates/domain/src/timeframe.rs`

- [ ] **Step 1: Write 2 failing tests**

In `crates/domain/src/timeframe.rs`, inside the existing `#[cfg(test)] mod tests` block, add these two tests after the existing ones:

```rust
#[test]
fn from_str_known_variant() {
    let tf: Timeframe = "M15".parse().unwrap();
    assert_eq!(tf, Timeframe::M15);
}

#[test]
fn from_str_unknown_returns_err() {
    let result = "invalid".parse::<Timeframe>();
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run to confirm compile failure**

```bash
cargo test -p domain 2>&1 | tail -10
```

Expected: compile error — `FromStr` not implemented for `Timeframe`.

- [ ] **Step 3: Add `FromStr` impl**

In `crates/domain/src/timeframe.rs`, add this impl after the existing `impl Timeframe { ... }` block:

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

- [ ] **Step 4: Run tests**

```bash
cargo test -p domain 2>&1 | tail -10
```

Expected:
```
test tests::as_api_str_spot_check   ... ok
test tests::from_str_known_variant  ... ok
test tests::from_str_unknown_returns_err ... ok
test tests::serde_round_trip        ... ok

test result: ok. 4 passed; 0 failed
```

- [ ] **Step 5: Run clippy**

```bash
cargo clippy -p domain -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/domain/src/timeframe.rs
git commit -m "feat(domain): add FromStr impl for Timeframe"
```

---

### Task 2: bin/hermes — Cargo.toml + main.rs

**Files:**
- Modify: `bin/hermes/Cargo.toml`
- Modify: `bin/hermes/src/main.rs`

- [ ] **Step 1: Update Cargo.toml**

Replace full contents of `bin/hermes/Cargo.toml`:

```toml
[package]
name    = "hermes"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow             = { workspace = true }
domain             = { workspace = true }
dotenvy            = { workspace = true }
engine             = { workspace = true }
mt5-client         = { workspace = true }
openrouter         = { workspace = true }
rust_decimal       = { workspace = true }
tokio              = { workspace = true }
tracing            = { workspace = true }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 2: Implement main.rs**

Replace full contents of `bin/hermes/src/main.rs`:

```rust
use std::time::Duration;

use anyhow::Context;
use engine::{EngineConfig, EngineOutcome};
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mt5_base_url  = std::env::var("MT5_BASE_URL").context("MT5_BASE_URL missing")?;
    let llm_model     = std::env::var("LLM_MODEL").context("LLM_MODEL missing")?;
    let symbols_raw   = std::env::var("SYMBOLS").context("SYMBOLS missing")?;
    let timeframe_str = std::env::var("TIMEFRAME").context("TIMEFRAME missing")?;
    let candle_count  = std::env::var("CANDLE_COUNT")
        .context("CANDLE_COUNT missing")?
        .parse::<u32>()
        .context("CANDLE_COUNT must be a u32")?;
    let risk_pct      = std::env::var("RISK_PCT")
        .context("RISK_PCT missing")?
        .parse::<Decimal>()
        .context("RISK_PCT must be a decimal (e.g. 0.01)")?;
    let cycle_secs    = std::env::var("CYCLE_SECS")
        .context("CYCLE_SECS missing")?
        .parse::<u64>()
        .context("CYCLE_SECS must be a u64")?;

    let timeframe = timeframe_str
        .parse::<domain::Timeframe>()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let symbols: Vec<String> = symbols_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if symbols.is_empty() {
        anyhow::bail!("SYMBOLS must contain at least one symbol");
    }

    let mt5    = mt5_client::Mt5Client::new(mt5_base_url);
    let llm    = openrouter::OpenRouterClient::new();
    let config = EngineConfig { timeframe, candle_count, risk_pct };

    tracing::info!(
        symbols    = ?symbols,
        timeframe  = ?config.timeframe,
        candle_count = config.candle_count,
        risk_pct   = %config.risk_pct,
        cycle_secs,
        "hermes starting"
    );

    loop {
        let start = tokio::time::Instant::now();

        let symbol_refs: Vec<&str> = symbols.iter().map(String::as_str).collect();

        match engine::run_once(&symbol_refs, &mt5, &llm, &llm_model, &config).await {
            Ok(EngineOutcome::Traded { symbol, action, volume, order }) => {
                tracing::info!(symbol, ?action, %volume, order, "trade executed");
            }
            Ok(EngineOutcome::NoSignal)    => tracing::debug!("no ICT signal on any symbol"),
            Ok(EngineOutcome::Hold)        => tracing::debug!("agents voted hold on all symbols"),
            Ok(EngineOutcome::NoApproval)  => tracing::debug!("risk rejected all candidates"),
            Err(e) => tracing::error!(error = %e, "cycle error, continuing"),
        }

        let elapsed = start.elapsed();
        let cycle   = Duration::from_secs(cycle_secs);
        if elapsed < cycle {
            tokio::time::sleep(cycle - elapsed).await;
        }
    }
}
```

- [ ] **Step 3: Build**

```bash
cargo build -p hermes 2>&1 | tail -15
```

Expected:
```
Compiling hermes v0.1.0 (...)
Finished `dev` profile [unoptimized + debuginfo] target(s) in ...
```

Zero errors. Warnings from unused deps in other crates are acceptable — only `hermes` warnings matter.

- [ ] **Step 4: Clippy**

```bash
cargo clippy -p hermes -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add bin/hermes/Cargo.toml bin/hermes/src/main.rs
git commit -m "feat(hermes): implement main binary with env config and trading loop"
```

---

## Success Criteria

- `cargo build -p hermes` → kompilasi sukses, zero errors
- `cargo test -p domain` → 4 tests pass (including 2 new `from_str` tests)
- `cargo clippy -p domain -p hermes -- -D warnings` → zero warnings
- Binary startup dengan env var missing → exit non-zero dengan pesan jelas
- `TIMEFRAME=invalid` → error "unknown timeframe: invalid"
- Trading loop: log outcome setiap siklus, tidur `CYCLE_SECS - elapsed` detik
