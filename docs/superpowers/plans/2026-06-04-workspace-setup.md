# Workspace Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mengubah single Rust package menjadi Cargo workspace dengan 7 library crates + 1 binary, semua internal dan external dependencies sudah di-wire, `cargo build --release` dan `cargo clippy` langsung hijau.

**Architecture:** Workspace root `Cargo.toml` mendaftarkan semua members dan menjadi satu-satunya tempat versi external deps di-pin. Tiap crate punya `Cargo.toml` sendiri yang hanya mendeklarasikan deps yang benar-benar dipakai, mengacu ke versi workspace dengan `{ workspace = true }`. Dependency direction mengikuti: `hermes` → `engine` → (`agents`, `risk`, `ict`, `mt5-client`) → (`openrouter`, `domain`). Tidak ada cycles.

**Tech Stack:** Rust edition 2024 (requires rustc ≥ 1.85), Cargo workspace, tokio, reqwest (rustls), serde, rust_decimal, chrono/chrono-tz, thiserror, anyhow, tracing.

---

## File Map

| File | Action | Keterangan |
|---|---|---|
| `Cargo.toml` | Modify | Hapus `[package]`, ganti dengan `[workspace]` + `[workspace.dependencies]` |
| `src/main.rs` | Delete | Digantikan oleh `bin/hermes/src/main.rs` |
| `.gitignore` | Create | `/target`, `.env`, `*.env` |
| `tests/fixtures/.gitkeep` | Create | Placeholder direktori fixtures |
| `crates/domain/Cargo.toml` | Create | `serde`, `rust_decimal`, `chrono`, `chrono-tz` |
| `crates/domain/src/lib.rs` | Create | Stub kosong |
| `crates/openrouter/Cargo.toml` | Create | `reqwest`, `serde_json`, `tokio`, `thiserror`, `tracing` |
| `crates/openrouter/src/lib.rs` | Create | Stub kosong |
| `crates/ict/Cargo.toml` | Create | `domain`, `thiserror`, `tracing` |
| `crates/ict/src/lib.rs` | Create | Stub kosong |
| `crates/mt5-client/Cargo.toml` | Create | `domain`, `reqwest`, `serde_json`, `tokio`, `thiserror`, `tracing` |
| `crates/mt5-client/src/lib.rs` | Create | Stub kosong |
| `crates/agents/Cargo.toml` | Create | `domain`, `openrouter`, `serde_json`, `tokio`, `thiserror`, `tracing` |
| `crates/agents/src/lib.rs` | Create | Stub kosong |
| `crates/risk/Cargo.toml` | Create | `domain`, `mt5-client`, `rust_decimal`, `thiserror`, `tracing` |
| `crates/risk/src/lib.rs` | Create | Stub kosong |
| `crates/engine/Cargo.toml` | Create | `agents`, `risk`, `ict`, `mt5-client`, `domain`, `tokio`, `thiserror`, `tracing` |
| `crates/engine/src/lib.rs` | Create | Stub kosong |
| `bin/hermes/Cargo.toml` | Create | `engine`, `anyhow`, `dotenvy`, `tokio`, `tracing-subscriber` |
| `bin/hermes/src/main.rs` | Create | async main minimal |

---

## Task 1: Workspace Manifest

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Ganti isi root `Cargo.toml`**

Hapus semua konten yang ada, tulis ulang dengan:

```toml
[workspace]
members = [
    "crates/domain",
    "crates/mt5-client",
    "crates/ict",
    "crates/openrouter",
    "crates/agents",
    "crates/risk",
    "crates/engine",
    "bin/hermes",
]
resolver = "2"

[workspace.dependencies]
# --- internal crates ---
domain     = { path = "crates/domain" }
mt5-client = { path = "crates/mt5-client" }
ict        = { path = "crates/ict" }
openrouter = { path = "crates/openrouter" }
agents     = { path = "crates/agents" }
risk       = { path = "crates/risk" }
engine     = { path = "crates/engine" }

# --- external ---
anyhow             = "1"
chrono             = { version = "0.4", features = ["serde"] }
chrono-tz          = "0.10"
dotenvy            = "0.15"
reqwest            = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
rust_decimal       = { version = "1", features = ["serde"] }
serde              = { version = "1", features = ["derive"] }
serde_json         = "1"
thiserror          = "2"
tokio              = { version = "1", features = ["full"] }
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 2: Hapus `src/` lama**

```bash
rm -rf src/
```

---

## Task 2: Foundation Crates — `domain` dan `openrouter`

Kedua crate ini tidak punya internal deps, jadi dibuat duluan.

**Files:**
- Create: `crates/domain/Cargo.toml`
- Create: `crates/domain/src/lib.rs`
- Create: `crates/openrouter/Cargo.toml`
- Create: `crates/openrouter/src/lib.rs`

- [ ] **Step 1: Buat `crates/domain/Cargo.toml`**

```toml
[package]
name    = "domain"
version = "0.1.0"
edition = "2024"

[dependencies]
chrono       = { workspace = true }
chrono-tz    = { workspace = true }
rust_decimal = { workspace = true }
serde        = { workspace = true }
```

- [ ] **Step 2: Buat `crates/domain/src/lib.rs`**

```rust
// Phase 1: domain types — Candle, Tick, Symbol, Position, AccountInfo, Timeframe
```

- [ ] **Step 3: Buat `crates/openrouter/Cargo.toml`**

```toml
[package]
name    = "openrouter"
version = "0.1.0"
edition = "2024"

[dependencies]
reqwest    = { workspace = true }
serde_json = { workspace = true }
thiserror  = { workspace = true }
tokio      = { workspace = true }
tracing    = { workspace = true }
```

- [ ] **Step 4: Buat `crates/openrouter/src/lib.rs`**

```rust
// Phase 4: OpenRouter (OpenAI-compatible) chat client + structured-output parsing
```

---

## Task 3: Second-Tier Crates — `ict` dan `mt5-client`

Kedua crate ini hanya depend ke `domain`.

**Files:**
- Create: `crates/ict/Cargo.toml`
- Create: `crates/ict/src/lib.rs`
- Create: `crates/mt5-client/Cargo.toml`
- Create: `crates/mt5-client/src/lib.rs`

- [ ] **Step 1: Buat `crates/ict/Cargo.toml`**

```toml
[package]
name    = "ict"
version = "0.1.0"
edition = "2024"

[dependencies]
domain    = { workspace = true }
thiserror = { workspace = true }
tracing   = { workspace = true }
```

- [ ] **Step 2: Buat `crates/ict/src/lib.rs`**

```rust
// Phase 3: ICT strategy primitives — FVG, OB, BOS/CHoCH, sweeps, premium/discount, OTE
```

- [ ] **Step 3: Buat `crates/mt5-client/Cargo.toml`**

```toml
[package]
name    = "mt5-client"
version = "0.1.0"
edition = "2024"

[dependencies]
domain     = { workspace = true }
reqwest    = { workspace = true }
serde_json = { workspace = true }
thiserror  = { workspace = true }
tokio      = { workspace = true }
tracing    = { workspace = true }
```

- [ ] **Step 4: Buat `crates/mt5-client/src/lib.rs`**

```rust
// Phase 2: typed async REST client for the mt5api bridge
```

---

## Task 4: Third-Tier Crates — `agents` dan `risk`

**Files:**
- Create: `crates/agents/Cargo.toml`
- Create: `crates/agents/src/lib.rs`
- Create: `crates/risk/Cargo.toml`
- Create: `crates/risk/src/lib.rs`

- [ ] **Step 1: Buat `crates/agents/Cargo.toml`**

```toml
[package]
name    = "agents"
version = "0.1.0"
edition = "2024"

[dependencies]
domain     = { workspace = true }
openrouter = { workspace = true }
serde_json = { workspace = true }
thiserror  = { workspace = true }
tokio      = { workspace = true }
tracing    = { workspace = true }
```

- [ ] **Step 2: Buat `crates/agents/src/lib.rs`**

```rust
// Phase 5: agent roles, prompts, and orchestrator/quorum aggregation
```

- [ ] **Step 3: Buat `crates/risk/Cargo.toml`**

```toml
[package]
name    = "risk"
version = "0.1.0"
edition = "2024"

[dependencies]
domain       = { workspace = true }
mt5-client   = { workspace = true }
rust_decimal = { workspace = true }
thiserror    = { workspace = true }
tracing      = { workspace = true }
```

- [ ] **Step 4: Buat `crates/risk/src/lib.rs`**

```rust
// Phase 6: position sizing, risk gate, and kill switch
```

---

## Task 5: Top-Tier Crate — `engine`

**Files:**
- Create: `crates/engine/Cargo.toml`
- Create: `crates/engine/src/lib.rs`

- [ ] **Step 1: Buat `crates/engine/Cargo.toml`**

```toml
[package]
name    = "engine"
version = "0.1.0"
edition = "2024"

[dependencies]
agents     = { workspace = true }
domain     = { workspace = true }
ict        = { workspace = true }
mt5-client = { workspace = true }
risk       = { workspace = true }
thiserror  = { workspace = true }
tokio      = { workspace = true }
tracing    = { workspace = true }
```

- [ ] **Step 2: Buat `crates/engine/src/lib.rs`**

```rust
// Phase 7: run loop — data → features → agents → gate → record
```

---

## Task 6: Binary — `bin/hermes`

**Files:**
- Create: `bin/hermes/Cargo.toml`
- Create: `bin/hermes/src/main.rs`

- [ ] **Step 1: Buat `bin/hermes/Cargo.toml`**

```toml
[package]
name    = "hermes"
version = "0.1.0"
edition = "2024"

[dependencies]
engine             = { workspace = true }
anyhow             = { workspace = true }
dotenvy            = { workspace = true }
tokio              = { workspace = true }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 2: Buat `bin/hermes/src/main.rs`**

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Ok(())
}
```

---

## Task 7: Cleanup dan Final Verification

**Files:**
- Create: `.gitignore`
- Create: `tests/fixtures/.gitkeep`

- [ ] **Step 1: Buat `.gitignore`**

```
/target
.env
*.env
```

- [ ] **Step 2: Buat `tests/fixtures/.gitkeep`**

```bash
mkdir -p tests/fixtures && touch tests/fixtures/.gitkeep
```

- [ ] **Step 3: Full build release**

```bash
cargo build --release
```

Expected: semua 8 members compile tanpa error.

- [ ] **Step 4: Clippy zero warnings**

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: tidak ada output warning atau error.

- [ ] **Step 5: Test suite**

```bash
cargo test
```

Expected: `running 0 tests` di setiap crate, zero failures.

- [ ] **Step 6: Verifikasi dependency tree tidak ada cycles**

```bash
cargo tree -p hermes
```

Expected: tree menunjukkan `hermes` → `engine` → crate-crate lain → `domain` di dasar. Tidak ada baris yang muncul dua kali di jalur yang sama (yang menunjukkan cycle).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore crates/ bin/ tests/
git commit -m "feat: scaffold cargo workspace with 7 crates + hermes binary

- workspace manifest dengan semua [workspace.dependencies] ter-pin
- 7 library crates: domain, mt5-client, ict, openrouter, agents, risk, engine
- binary: bin/hermes dengan async main stub
- semua internal path-deps sudah di-wire sesuai dependency graph
- cargo build --release dan clippy clean"
```
