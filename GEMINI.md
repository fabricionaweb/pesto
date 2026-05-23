# Project Instructions (GEMINI.md)

This file contains mandates and conventions for Gemini CLI. These instructions take precedence over general defaults.

## Core Mandates

- **Language:** The official language of this project is **English**. All code, comments, documentation, commit messages, and identifiers must be written in English.
- **Performance:** Speed is the priority. Use streaming I/O, avoid needless allocations/buffer copies, and minimize lock contention.
- **Concurrency:** Post articles using a pool of concurrent NNTP connections.
- **Error Handling:** Network, authentication, and I/O errors must produce actionable messages, not panics.

## Tech Stack

- **Runtime:** `tokio`
- **TLS:** `rustls` + `tokio-rustls`
- **CLI:** `clap` (derive)
- **Config:** `serde` + `toml`
- **Errors:** `anyhow` (binary), dedicated types (library)

## Development Workflow

Before every commit, the following must pass:
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`

## Conventions

- **Formatting:** Use default `cargo fmt`.
- **Commits:** Short imperative messages.
- **Specifications:** Cite yEnc/NNTP specs in comments when making relevant changes.

Refer to `CLAUDE.md` for the full guide and `ROADMAP.md` for project status.
