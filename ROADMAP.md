# Roadmap — `pesto`

Fast, lean Usenet poster in Rust. Inspired by `nyuu`, with only the essentials.
Each phase must leave the program in a working, testable state.

---

## Completed ✅

| Phase | Topic |
|-------|-------|
| 0 | Foundation — workspace, CLI skeleton, config structs, logging |
| 1 | yEnc encoder — `encode_into`, CRC32, segmentation, headers |
| 2 | Basic NNTP — TCP connection, `POST`, `240` response |
| 3 | TLS & Auth — `rustls`, `AUTHINFO USER/PASS`, env-var credentials |
| 4 | Concurrent posting — connection pool, MPSC work queue, Ctrl-C |
| 5 | NZB generation — XML writer, Message-ID capture, file grouping |
| 6 | Config file — TOML load, CLI-override merge, multi-group |
| 7 | PAR2 foundation — GF(2¹⁶), Cauchy matrix, packet serialization |
| 8 | PAR2 advanced — MD5 hashing, single-pass parity, AVX2/SSSE3 SIMD |
| 9 | Local archive & obfuscation — RAR/7z, filename randomisation, passwords |
| 10 | Metadata & hooks — `.nfo` generation, post-hooks, Newznab, Discord |
| 11 | Error resilience — retry/backoff, resume state file, STAT verification |
| 12 | Performance — double-buffered reader, buffer pool, Rayon, rate limiting |
| 13 | Polish & UI — ANSI multi-bar, JSON-L mode, setup wizard, sparklines |
| 20 | Modularisation — split wizard, TUI, PAR2 worker, config into sub-modules |
| 21a | Cargo workspace — `parmesan` extracted to `crates/parmesan` |
| 21b | API decoupling — removed NNTP terminology, generic `Read`-based API |
| 21c | Benchmarking — micro-benchmarks in library, `#[inline]` tuning, docs |

---

## In Progress

### Phase 21d — Publish `parmesan` to crates.io

- [ ] Version the library independently from `pesto`.
- [ ] Publish `parmesan-par2` to crates.io.
- [ ] Switch `pesto` to depend on the published crate (or keep workspace path).

See [`crates/parmesan/ROADMAP.md`](crates/parmesan/ROADMAP.md) for the full
`parmesan` roadmap.

---

## Next — Phase 22+: Complete PAR2 Tooling

The resource/geometry flags from the original Phase 22 plan are **already
implemented**. The focus now is on verify/repair, input flexibility, volume
layout control, and documentation.

Details live in [`crates/parmesan/ROADMAP.md`](crates/parmesan/ROADMAP.md).

---

## Phase 23 — Interactive TUI (Ratatui)

### 23a — Dashboard layout
- [ ] Replace current ANSI output with a `ratatui` layout.
- [ ] Tabs: `Progress`, `Logs`, `Connections`, `PAR2 Status`.
- [ ] Real-time throughput graph (`Canvas` or `Sparkline` widget).

### 23b — Interactive controls
- [ ] Pause/resume upload via keyboard.
- [ ] Adjust connection count at runtime.
- [ ] Scrollable, filterable log buffer.

---

## Phase 24 — Hot-Path Serialization: Scatter-Gather POST

Eliminate the redundant full-article copy that `Article::serialize()` currently
produces before every NNTP `POST`.

### Background

`serialize()` allocates a new `Vec<u8>` (~768 KB) per article by concatenating
headers and the yEnc body. This copy is unnecessary: the socket can receive two
disjoint buffers in a single syscall via scatter-gather I/O.

### 24a — Vectored writes on the NNTP connection

- [x] Replace `Connection::post(&[u8])` with `Connection::post_parts(&[u8], &[u8])`.
- [x] Use sequential `write_all` calls (coalesced by the `BufWriter` from 24b)
  to send headers + yEnc body without copying the body.
- [x] Keep `Article::serialize()` for tests; production path uses `build_headers()`.
- [x] The body is written without dot-stuffing because yEnc encoding already
  escapes `'.'` at line start (yEnc spec §4).

### 24b — TLS write buffering

- [x] Wrap the TLS stream in a `BufWriter` sized to ≥ 1 article to allow the
  TLS layer to coalesce small header writes with the body in one record,
  reducing syscall count and TLS fragmentation overhead.

---

## Phase 25 — NNTP Pipelining

Post multiple articles without waiting for the `240 Article received` response
of the previous one. This halves round-trip latency cost per article on
high-latency links (>50 ms RTT).

### 25a — Pipeline depth N

- [x] Send up to N `POST` commands and bodies back-to-back on the same
  connection before reading any responses.
- [x] Collect responses in order (NNTP responses arrive in command order).
- [x] On failure mid-pipeline, mark remaining articles as failed and retry the
  batch on the next attempt with `slot.invalidate()`.
- [x] Expose `--pipeline-depth` CLI flag and `posting.pipeline_depth` config
  option (default: 1; recommended 4–8 for high-latency servers).
- [x] Pipelining is automatically disabled when `--verify` is active (STAT
  after each article is incompatible with batched response reads).

### 25b — Adaptive pipeline depth

- [ ] Measure per-article RTT during warm-up phase.
- [ ] Automatically compute optimal pipeline depth:
  `depth = ceil(RTT / article_encode_time)`.
- [ ] Cap at server-side queue limit (detect `441 Too many articles` responses).

---

## Phase 26 — yEnc SIMD Encoder

Replace the byte-at-a-time yEnc loop with a SIMD-accelerated implementation
that processes 16–32 bytes per cycle.

Complexity levels, in order: scalar correctness → SSSE3 (16-byte) →
AVX2 (32-byte) → buffer pre-computation. Each level uses the previous
level's tests as a golden reference before any SIMD code is merged.

### 26a — Scalar baseline with full test coverage *(low complexity)*

- [ ] Extract the yEnc encode loop into a pure function `encode_scalar(input: &[u8], output: &mut Vec<u8>)` with no side-effects.
- [ ] Unit tests covering:
  - All four special bytes (`0x00`, `0x0A`, `0x0D`, `0x3D`) at every position (first, middle, last, consecutive).
  - Positional escapes: space (`0x20`) and tab (`0x09`) at line start (col 0) and at the `line_length` boundary.
  - Wrap-around at exactly `line_length` bytes — verify `\r\n` insertion.
  - Full 256-byte round-trip (`encode` → `decode` identity test).
  - CRC32 value matches reference vectors from the yEnc spec.
- [ ] Property-based test with `proptest` or `quickcheck`: for any `input: Vec<u8>`, `decode(encode(input)) == input`.
- [ ] Micro-benchmark for the scalar path in `benches/yenc.rs` (establishes the baseline to beat).

### 26b — SSSE3 baseline (x86-64) *(medium complexity)*

- [ ] Add a `yenc_ssse3` feature gate (enabled on `x86_64` targets by default).
- [ ] Implement the 16-byte-wide inner loop:
  - `_mm_add_epi8(chunk, splat(42))` — shift all bytes by 42.
  - Compute escape mask: identify `0x00`, `0x0A`, `0x0D`, `0x3D` lanes.
  - Handle positional escapes (space/tab at line boundaries) as scalar epilogue.
  - Emit escaped bytes and advance output pointer.
- [ ] Scalar fallback for the tail (< 16 bytes) and line-boundary regions.
- [ ] Re-run the full 26a test suite against the SSSE3 path (feature-gated `#[cfg]` test).
- [ ] Add SSSE3 vs scalar throughput comparison to the existing benchmark.

### 26c — AVX2 (256-bit) path *(medium-high complexity)*

- [ ] Extend to 32-byte-wide loop using AVX2 intrinsics.
- [ ] Runtime dispatch: detect `avx2` CPU feature via `std::is_x86_feature_detected!`.
- [ ] Re-run the full 26a test suite against the AVX2 path.
- [ ] Update `SimdPath` enum in `parmesan` to share the dispatch pattern if applicable.

### 26d — Line-length pre-computation *(high complexity)*

- [ ] Pre-compute exact output size per input chunk (accounting for escapes and
  `\r\n` insertions) to reserve the output buffer precisely, avoiding `push`
  reallocations inside the SIMD loop.
- [ ] Verify that the pre-computed size matches the actual output size in all 26a tests.

---

## Phase 32 — Future Ideas (Unscheduled)

Concepts to evaluate later. Not committed to any timeline.

| Idea | Summary |
|------|---------|
| RAM auto-cap | Cap buffer pools based on available system memory to prevent OOM |
| Dynamic connection scaling | Reduce connections under memory or TCP pressure |
| CPU topology awareness | Tune `rayon` pool to physical vs logical core count |
| Disk pre-flight | Verify free space before compression/PAR2 starts |
| In-memory mode | Skip temp files for small payloads that fit in RAM |
| `O_DIRECT` reads | Bypass page cache on Linux for huge files |
| `mmap` fast-path | `mmap` + `MADV_SEQUENTIAL` for massive file reads |
| Adaptive buffering | Grow/shrink buffer pool based on upload/read speed delta |
| Lock-free buffer pool | Replace `Mutex<Vec<_>>` pool with `SegQueue` to eliminate contention at high connection counts |
| Connection health scoring | Track per-server error rates passively; prefer healthy servers without hard failover |
| Warm reconnection | Pre-connect to the next failover server in background so TLS handshake cost is not paid on the hot path |
