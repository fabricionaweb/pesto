# pesto

Fast, lean Usenet poster, written in Rust.

It takes files and directories, encodes them with yEnc, posts the articles to
Usenet groups over NNTP, and generates an `.nzb` file. Inspired by
[`nyuu`](https://github.com/animetosho/Nyuu), but with a minimal scope: just
the basics, executed extremely fast.

It will be integrated into the posting flow of the `upapasta` program.

## Status

The MVP is complete: yEnc encoding, parallel TLS posting and `.nzb` generation
all work end to end, plus pure-Rust PAR2 recovery, posting obfuscation and
recursive directory uploads. See [`ROADMAP.md`](ROADMAP.md) for what comes next.

## Build

Requires the Rust toolchain (install via <https://rustup.rs>).

```bash
cargo build --release
```

The optimized binary is written to `target/release/pesto`.

## Usage

First-time setup — create a config file with the guided wizard:

```bash
pesto --config
```

This writes `~/.config/pesto/config.toml` (or `$XDG_CONFIG_HOME/pesto/`).
`pesto` loads that file automatically, so afterwards posting is just:

```bash
pesto movie.mkv
```

Running `pesto` with no arguments prints a short orientation screen. Any
config value can still be overridden on the command line, and an explicit
file can be loaded with `pesto --config <PATH> ...`. See
[`config.example.toml`](config.example.toml) for every option.

### Posting a directory

A `PATH` argument may be a directory — a TV-show season, or any folder with
nested subfolders:

```bash
pesto ./MyShow.S01/
```

The directory is walked recursively and the whole tree is posted as one
upload. The folder structure is preserved in the `.nzb` and the PAR2 metadata,
so a downloader rebuilds the original layout — including nested subfolders —
on repair. Files starting with `.` are included; symlinks inside the tree are
skipped. With no `--out`, the `.nzb` is named after the root folder
(`MyShow.S01.nzb`).

### Without a config file

Everything can be passed as flags instead:

```bash
pesto \
  --host news.example.com --port 563 \
  --username alice --password secret \
  --from 'alice <alice@example.com>' \
  --groups alt.binaries.test \
  --connections 10 \
  --out upload.nzb \
  movie.mkv
```

### Flags

| Flag | Description |
|------|-------------|
| `-c`, `--config [PATH]` | Load a TOML config file; with no value, run the setup wizard |
| `--host <HOST>` | NNTP server hostname |
| `--port <PORT>` | NNTP server port (default 563) |
| `--no-ssl` | Disable TLS |
| `--connections <N>` | Number of parallel connections (default 4) |
| `--retry-delay <SECS>` | Seconds between failed post attempts (default 1) |
| `--username <USER>` | Authentication username |
| `--password <PASS>` | Authentication password |
| `--from <ADDRESS>` | `From` header; omitted means a random identity per run |
| `--groups <G,...>` | Newsgroups to post to (comma-separated) |
| `--article-size <BYTES>` | Target size of each segment (default 768000) |
| `--line-length <CHARS>` | yEnc line length (default 128) |
| `--retries <N>` | Post attempts per segment (default 3) |
| `-o`, `--out <PATH>` | Path of the `.nzb` file to write |
| `--obfuscate[=MODE]` | Obfuscation mode: `none`, `subject` or `full` (bare flag = `full`) |
| `--par2 <PERCENT>` | PAR2 recovery data percentage (default 10, 0 disables) |
| `--par2-only` | Only generate PAR2 files; do not post |
| `--dry-run` | Encode only, never touch the network |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | All segments posted |
| `1` | One or more segments failed |
| `130` | Interrupted with Ctrl-C |

On Ctrl-C, `pesto` stops taking new segments, lets in-flight ones finish, and
still writes an `.nzb` for whatever was posted.

### Obfuscation

`--obfuscate` has three modes:

- **`none`** (default) — the real file name appears in the subject and the
  yEnc header.
- **`subject`** — random subject, but the yEnc `name=` field keeps the real
  file name, so a standard download client still names the file correctly.
  This hides the post from public indexers.
- **`full`** — random subject *and* random yEnc `name=`. Nothing on the wire
  reveals the real name. Recover it from the generated `.nzb` (the `name`
  attribute of the `<file>` element) or, when posted alongside PAR2 files,
  let PAR2 rename the files by content hash.

A bare `--obfuscate` (no value) means `full`.

## Development

```bash
cargo test                  # unit + integration tests
cargo clippy -- -D warnings
cargo fmt
```

## License

MIT
