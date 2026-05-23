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

### 26a — Scalar baseline with full test coverage *(low complexity)* ✅

- [x] Extract the yEnc encode loop into `pub fn encode_scalar(out: &mut Vec<u8>, data: &[u8], line_len: usize)`.
- [x] 30 unit tests: all four critical bytes at first/middle/last/consecutive positions,
  positional escapes for space/tab/dot at line boundaries, exact wrap-around, 256-byte round-trip, CRC32 check values.
- [x] Micro-benchmark in `benches/yenc.rs` — baseline ~515 MB/s.

### 26b — SSSE3 baseline (x86-64) *(medium complexity)* ✅

- [x] `pub fn encode_ssse3`: runtime dispatch via `is_x86_feature_detected!("ssse3")`.
- [x] 16-byte inner loop: `_mm_add_epi8` shift, 4× `_mm_cmpeq_epi8` escape mask, `_mm_movemask_epi8`; zero-mask fast path writes 16 bytes direct.
- [x] Line-start and line-end bytes always scalar (positional escape rules); only critical bytes need escaping in the middle zone.
- [x] 8 golden-reference tests verify SSSE3 output matches `encode_scalar` exactly (750 KB payload, all byte values, boundary positions, short line lengths).
- [x] Benchmark: **~1680 MB/s** (≈3.2× scalar).

### 26c — AVX2 (256-bit) path *(medium-high complexity)* ✅

- [x] `pub fn encode_avx2`: 32-byte AVX2 chunks in the middle zone, SSSE3 16-byte remainder, scalar tail.
- [x] `pub fn encode()` dispatcher: AVX2 > SSSE3 > scalar, selected once per call via `is_x86_feature_detected!`. `encode_part` now calls `encode()`.
- [x] 9 golden-reference tests verify AVX2 output matches `encode_scalar` exactly.
- [x] Benchmark: **~1470 MB/s** (≈2.8× scalar). For `line_len=128` the safe zone is 126 bytes (3 AVX2 + 1 SSSE3 chunks), so SSSE3 edges it out at this line length; longer lines favour AVX2.

### 26d — Buffer pre-reservation *(high complexity)* ✅

- [x] Add `pub fn encoded_size(data, line_len) -> usize`: exact scalar count of
  output bytes (escaped pairs + CRLF termintors). Useful for callers that need
  the buffer size before encoding (NZB builders, fixed-size writers).
- [x] Replace per-chunk `reserve(16/32)` calls inside SIMD loops with a single
  O(1) upper-bound reserve at function entry:
  `data.len() * 2 + (data.len() / line_len + 1) * 2` (always sufficient).
  Calling `encoded_size()` inside SIMD encodes would add a full O(n) scalar
  pass and eliminate the SIMD speedup — O(1) upper bound is the right trade-off.
- [x] 6 new tests verify `encoded_size` matches actual output length for all
  boundary conditions and a 750 KB payload.

---

## Phase 27 — yEnc Encoder: AVX2 Correctness & line_len Scaling

Target: exceed nyuu's documented yEnc throughput (~1.2 GB/s AVX2 at
`line_len=128`) and reach 2–3 GB/s at `line_len=256`. All changes must keep
the full Phase 26 golden-reference test suite green.

### 27a — Fix AVX2 256→128-bit register mixing *(low complexity)*

The current `encode_avx2_impl` falls back to 128-bit SSSE3 instructions
(`_mm_cmpeq_epi8`, `_mm_storeu_si128`) for the <32-byte safe-zone remainder
within the same function. CPUs detect this 256→128 transition and insert
implicit VZEROUPPER-equivalent stalls, which is why AVX2 benchmarks 12%
*slower* than SSSE3 for `line_len=128`.

- [ ] Remove the inline 128-bit block from `encode_avx2_impl`; use scalar
  directly for the safe-zone tail after the 32-byte chunks.
- [ ] Verify AVX2 now outperforms SSSE3 at `line_len=128`.
- [ ] Re-run full Phase 26 test suite.

### 27b — Dispatcher: line_len-aware path selection *(low complexity)*

- [ ] In `pub fn encode()`, prefer SSSE3 when `line_len < 48` (safe zone
  too narrow to fit even one AVX2 chunk) and AVX2 otherwise.
- [ ] Add a test asserting the selected path for representative `line_len`
  values (e.g. 1, 32, 48, 64, 128, 256).

### 27c — Benchmark and validate at line_len=256 *(low complexity)*

`line_len=256` gives a safe zone of 254 bytes: 7 AVX2 chunks (224 bytes) vs
3 today. This is where AVX2 earns its width.

- [ ] Add `line_len=256` rows to `benches/yenc.rs` for all four paths.
- [ ] Add nyuu's documented yEnc throughput (~1.2 GB/s, `line_len=128`) as a
  printed reference line so every benchmark run shows the target to beat.
- [ ] Expected outcome: AVX2 at `line_len=256` ≥ 2 GB/s, exceeding nyuu.

### 27d — DEFAULT_LINE_LENGTH: evaluate raising to 256 *(medium complexity)*

`line_len=128` is historical (yEnc draft spec, 2001). Many modern servers
accept 256. nyuu itself defaults to 128 but supports 256.

- [ ] Survey what Usenet indexers and servers actually accept today.
- [ ] If compatible: raise `DEFAULT_LINE_LENGTH` to 256 and update config
  documentation. Keep 128 available via `--line-length` flag.
- [ ] Re-run integration tests and `encode_part` golden-reference tests.

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
