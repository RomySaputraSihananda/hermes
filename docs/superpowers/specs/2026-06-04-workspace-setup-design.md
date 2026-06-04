# Workspace Setup — Design Spec

**Date:** 2026-06-04
**Phase:** 0 (prerequisite untuk semua fase berikutnya)
**Status:** Approved

---

## Tujuan

Mengubah single Rust package `hermes` menjadi Cargo workspace dengan 7 library crates + 1 binary, semua internal dependencies sudah di-wire, semua external dependencies di-pin di workspace level, dan `cargo build --release` + `cargo clippy --all-targets -- -D warnings` langsung hijau.

---

## Struktur Direktori

```
hermes/
├── Cargo.toml                  ← workspace manifest
├── .gitignore
├── crates/
│   ├── domain/                 ← core types, zero internal deps
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── mt5-client/             ← typed async REST client untuk mt5api
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── ict/                    ← ICT strategy primitives, pure/deterministic
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── openrouter/             ← OpenRouter LLM chat client
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── agents/                 ← agent roles + orchestrator/quorum
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── risk/                   ← position sizing + risk gate + kill switch
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── engine/                 ← run loop, wiring semua crate
│       ├── Cargo.toml
│       └── src/lib.rs
├── bin/
│   └── hermes/                 ← binary entrypoint
│       ├── Cargo.toml
│       └── src/main.rs
└── tests/
    └── fixtures/               ← recorded JSON responses (kosong, diisi per fase)
```

---

## Dependency Graph

```
hermes (bin)
    └── engine
            ├── agents
            │     ├── openrouter
            │     └── domain
            ├── risk
            │     ├── domain
            │     └── mt5-client
            │             └── domain
            ├── ict
            │     └── domain
            └── mt5-client

domain      ← tidak ada internal deps
openrouter  ← tidak ada internal deps
```

Tidak ada cycles. `ict` dan `domain` tidak depend ke crate lain di workspace.

---

## External Dependencies per Crate

Semua versi di-pin di `[workspace.dependencies]`. Tiap crate pakai `{ workspace = true }`.

| Crate | External deps |
|---|---|
| `domain` | `serde` (derive), `rust_decimal` (serde), `chrono`, `chrono-tz` |
| `mt5-client` | `reqwest` (rustls-tls), `serde_json`, `tokio` (full), `thiserror`, `tracing` |
| `ict` | `thiserror`, `tracing` |
| `openrouter` | `reqwest` (rustls-tls), `serde_json`, `tokio` (full), `thiserror`, `tracing` |
| `agents` | `serde_json`, `tokio` (full), `thiserror`, `tracing` |
| `risk` | `rust_decimal`, `thiserror`, `tracing` |
| `engine` | `tokio` (full), `thiserror`, `tracing` |
| `hermes` (bin) | `tokio` (full), `anyhow`, `tracing-subscriber` (env-filter), `dotenvy` |

---

## Stub Content

### Library crates (`src/lib.rs`)

Satu baris komentar yang menjelaskan tujuan crate. Tidak ada code, tidak ada `pub use`. Cukup untuk compile.

### Binary (`bin/hermes/src/main.rs`)

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Ok(())
}
```

### `.gitignore`

Entry standar Rust: `/target`, `.env`, `*.env`.

---

## Migrasi dari State Sekarang

State saat ini: single package `hermes` dengan `src/main.rs` di root.

Yang perlu dilakukan:
1. Root `Cargo.toml` — hapus `[package]` + `[dependencies]`, ganti dengan `[workspace]` manifest
2. `src/` di root — hapus (digantikan oleh `bin/hermes/src/main.rs`)
3. Buat semua direktori dan file baru sesuai struktur di atas

---

## Success Criteria

- `cargo build --release` → hijau, zero errors
- `cargo clippy --all-targets -- -D warnings` → zero warnings
- `cargo test` → zero tests, zero failures
- Semua 7 crates + 1 binary terdaftar sebagai workspace members
- Dependency direction tidak ada cycles (verifiable dengan `cargo tree`)
