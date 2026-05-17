//! `pesto` — fast, lean Usenet poster.
//!
//! Parses the CLI, resolves the configuration, posts the given files to Usenet
//! and writes an `.nzb` file describing the result.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use pesto::config::{self, Config, FileConfig, ObfuscateMode, Overrides};

/// One-line summary shown at the top of `--help`.
const ABOUT: &str = "Fast, lean Usenet poster: yEnc-encode files, post over NNTP, emit an .nzb.";

/// Extended description shown by `pesto --help`.
const LONG_ABOUT: &str = "\
pesto posts files to Usenet. It yEnc-encodes each file, uploads the articles
over parallel NNTP connections and writes an .nzb describing what was posted.

A PATH argument may be a directory: it is walked recursively and the whole
tree is posted as one upload, with the folder structure preserved in the .nzb
and PAR2 metadata.

Server and credentials are read from a TOML config file. If --config is not
given, pesto loads $XDG_CONFIG_HOME/pesto/config.toml (or, failing that,
~/.config/pesto/config.toml) so a single setup serves every run. Create that
file interactively with `pesto --config`.

Any config value can be overridden by the matching flag below.";

/// Examples printed after the option list.
const AFTER_HELP: &str = "\
EXAMPLES:
  pesto movie.mkv                 post one file using the saved config
  pesto ./Season01/               post a whole directory, structure preserved
  pesto --config                  create the config file with a guided wizard
  pesto --out up.nzb a.bin b.bin  post two files and write an .nzb
  pesto --par2 15 movie.mkv       post with 15% PAR2 recovery data
  pesto --dry-run movie.mkv       encode only, never touch the network

By default pesto posts under a freshly generated random identity. Set
[posting].from (or --from) only if you need a fixed one.";

#[derive(Parser, Debug)]
#[command(
    name = "pesto",
    version,
    about = ABOUT,
    long_about = LONG_ABOUT,
    after_help = AFTER_HELP
)]
struct Cli {
    /// TOML config file to load. With no value (`pesto --config`), launch the
    /// interactive setup wizard instead. When omitted, the default config
    /// path is used if it exists.
    #[arg(short, long, value_name = "PATH", num_args = 0..=1)]
    config: Option<Option<PathBuf>>,

    /// NNTP server hostname [config: server.host].
    #[arg(long, value_name = "HOST")]
    host: Option<String>,

    /// NNTP server port [config: server.port, default 563].
    #[arg(long, value_name = "PORT")]
    port: Option<u16>,

    /// Disable TLS; connect in plaintext [config: server.ssl].
    #[arg(long)]
    no_ssl: bool,

    /// Number of parallel connections [config: server.connections, default 4].
    #[arg(long, value_name = "N")]
    connections: Option<usize>,

    /// Authentication username [config: auth.username].
    #[arg(long, value_name = "USER")]
    username: Option<String>,

    /// Authentication password [config: auth.password].
    #[arg(long, value_name = "PASS")]
    password: Option<String>,

    /// `From` header for posted articles; omitted means a random identity
    /// [config: posting.from].
    #[arg(long, value_name = "ADDRESS")]
    from: Option<String>,

    /// Newsgroups to post to (repeat or comma-separate) [config: posting.groups].
    #[arg(long, value_name = "GROUP", value_delimiter = ',')]
    groups: Vec<String>,

    /// Target size of each article body, in bytes
    /// [config: posting.article_size, default 768000].
    #[arg(long, value_name = "BYTES")]
    article_size: Option<usize>,

    /// yEnc line length, in encoded characters
    /// [config: posting.line_length, default 128].
    #[arg(long, value_name = "CHARS")]
    line_length: Option<usize>,

    /// Post attempts per segment before it is marked failed
    /// [config: posting.retries, default 3].
    #[arg(long, value_name = "N")]
    retries: Option<u32>,

    /// Seconds to wait between failed post attempts
    /// [config: server.retry_delay, default 1].
    #[arg(long, value_name = "SECS")]
    retry_delay: Option<u64>,

    /// Path of the `.nzb` file to write [config: output.nzb].
    #[arg(short, long, value_name = "PATH")]
    out: Option<PathBuf>,

    /// Obfuscation mode: `none`, `subject` or `full`. A bare `--obfuscate`
    /// means `full` [config: posting.obfuscate, default none].
    #[arg(long, value_name = "MODE", value_enum, num_args = 0..=1, default_missing_value = "full")]
    obfuscate: Option<ObfuscateMode>,

    /// Percentage of PAR2 recovery data to generate; 0 disables it
    /// [config: posting.par2, default 10].
    #[arg(long, value_name = "PERCENT")]
    par2: Option<u8>,

    /// Only generate PAR2 files next to the sources; do not post.
    #[arg(long)]
    par2_only: bool,

    /// Skip network posting and just measure generation speed.
    #[arg(long)]
    dry_run: bool,

    /// Files or directories to post. A directory is walked recursively and
    /// every file inside it is posted, keeping the folder structure.
    #[arg(value_name = "PATH")]
    files: Vec<PathBuf>,
}

impl Cli {
    /// Build config [`Overrides`] from the parsed flags.
    fn overrides(&self) -> Overrides {
        Overrides {
            host: self.host.clone(),
            port: self.port,
            // `--no-ssl` is the only TLS flag; absent means "defer to config".
            ssl: if self.no_ssl { Some(false) } else { None },
            connections: self.connections,
            username: self.username.clone(),
            password: self.password.clone(),
            from: self.from.clone(),
            groups: if self.groups.is_empty() {
                None
            } else {
                Some(self.groups.clone())
            },
            article_size: self.article_size,
            line_length: self.line_length,
            retries: self.retries,
            retry_delay: self.retry_delay,
            obfuscate: self.obfuscate,
            dry_run: if self.dry_run { Some(true) } else { None },
            par2: self.par2,
            par2_only: if self.par2_only { Some(true) } else { None },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // `pesto --config` with no value: launch the interactive setup wizard.
    if matches!(cli.config, Some(None)) {
        return run_wizard();
    }

    // `pesto` with nothing to post: show the orientation screen and stop.
    if cli.files.is_empty() {
        print_welcome();
        return Ok(());
    }

    // Resolve which config file to read: an explicit `--config PATH`, else the
    // default path when it exists, else nothing (flags must supply the rest).
    let (file_config, nzb_default) = match &cli.config {
        Some(Some(path)) => (FileConfig::load(path)?, None),
        _ => match config::default_config_path().filter(|p| p.exists()) {
            Some(path) => {
                eprintln!("using config: {}", path.display());
                let fc = FileConfig::load(&path)?;
                let nzb = fc.output.nzb.clone();
                (fc, nzb)
            }
            None => (FileConfig::default(), None),
        },
    };
    let nzb_default = nzb_default.or_else(|| file_config.output.nzb.clone());

    let config = Config::resolve(file_config, cli.overrides())?;

    // Expand the path arguments: directories are walked recursively so the
    // whole tree is posted as one upload.
    let inputs = pesto::walk::expand_inputs(&cli.files)?;
    let (file_count, folder_count, total_bytes) = upload_summary(&inputs);

    // Install the terminal progress panel. The poster only emits events; the
    // renderer task owns all terminal drawing and is awaited once posting ends.
    let (progress_tx, renderer) = pesto::progress::spawn_terminal_renderer();
    let outcome =
        pesto::poster::post_files_with_progress(&config, &inputs, Some(progress_tx)).await?;
    let _ = renderer.await;

    if config.par2_only {
        println!("PAR2 generation complete.");
    } else {
        println!("posted {} segment(s)", outcome.segments.len());
    }

    // Aggregate view of what was uploaded across the whole tree.
    let files_word = if file_count == 1 { "file" } else { "files" };
    let size = pesto::progress::format_size(total_bytes);
    if folder_count > 0 {
        let folders_word = if folder_count == 1 {
            "subfolder"
        } else {
            "subfolders"
        };
        println!("upload: {file_count} {files_word} · {folder_count} {folders_word} · {size}");
    } else {
        println!("upload: {file_count} {files_word} · {size}");
    }

    if outcome.cancelled {
        eprintln!("interrupted — stopped before posting every requested segment");
    }
    if !outcome.failures.is_empty() {
        eprintln!("{} segment(s) failed:", outcome.failures.len());
        for failure in &outcome.failures {
            eprintln!("  - {failure}");
        }
    }

    // `.nzb` destination: `--out` wins, then the config's output.nzb, then —
    // for a directory upload — a name derived from the root folder.
    let out = cli
        .out
        .clone()
        .or_else(|| nzb_default.map(PathBuf::from))
        .or_else(|| upload_root(&inputs).map(|root| PathBuf::from(format!("{root}.nzb"))));
    if let Some(out) = &out {
        if !config.par2_only {
            if outcome.segments.is_empty() {
                eprintln!("no segments posted — skipping nzb output");
            } else {
                let xml = pesto::nzb::generate(&config.from, &config.groups, &outcome.segments);
                tokio::fs::write(out, xml)
                    .await
                    .with_context(|| format!("writing nzb file `{}`", out.display()))?;
                println!("wrote nzb: {}", out.display());
            }
        }
    }

    // Exit codes: 130 for an interrupt, 1 for any failed segment, 0 otherwise.
    if outcome.cancelled {
        std::process::exit(130);
    }
    if !outcome.failures.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// The single root folder shared by every input, or `None` for loose files
/// or a mix of roots. Used to name the `.nzb` after the uploaded directory.
fn upload_root(inputs: &[pesto::walk::InputFile]) -> Option<String> {
    let mut root: Option<&str> = None;
    for input in inputs {
        let (candidate, _) = input.name.split_once('/')?;
        match root {
            Some(existing) if existing != candidate => return None,
            _ => root = Some(candidate),
        }
    }
    root.map(str::to_string)
}

/// Aggregate the upload as `(file count, subfolder count, total bytes)`.
/// A subfolder is any directory *below* a root folder (its relative path
/// contains a `/`); the root folder itself and loose files contribute none.
fn upload_summary(inputs: &[pesto::walk::InputFile]) -> (usize, usize, u64) {
    let mut subfolders = std::collections::BTreeSet::new();
    let mut bytes = 0u64;
    for input in inputs {
        let components: Vec<&str> = input.name.split('/').collect();
        let mut prefix = String::new();
        for component in &components[..components.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if prefix.contains('/') {
                subfolders.insert(prefix.clone());
            }
        }
        if let Ok(metadata) = std::fs::metadata(&input.path) {
            bytes += metadata.len();
        }
    }
    (inputs.len(), subfolders.len(), bytes)
}

/// Print the orientation screen shown when `pesto` is run with no files.
fn print_welcome() {
    let cfg = config::default_config_path();
    let cfg_exists = cfg.as_deref().map(Path::exists).unwrap_or(false);

    println!("pesto — fast, lean Usenet poster\n");
    println!("Getting started:");
    println!("  pesto <PATH>...     post files or directories to Usenet");
    println!("  pesto --config      create your config with a guided wizard");
    println!("  pesto --help        show every option in detail\n");

    match (&cfg, cfg_exists) {
        (Some(path), true) => println!("Config found: {}", path.display()),
        (Some(path), false) => {
            println!("No config yet. Run `pesto --config` to create one at:");
            println!("  {}", path.display());
        }
        (None, _) => println!(
            "Set $HOME or $XDG_CONFIG_HOME so pesto can locate a config file,\n\
             or pass every setting as a flag (see `pesto --help`)."
        ),
    }
}

/// Run the interactive setup wizard: prompt for the essential settings and
/// write them to the default config path.
fn run_wizard() -> Result<()> {
    let path = config::default_config_path()
        .context("cannot locate a config directory: set $HOME or $XDG_CONFIG_HOME")?;

    println!("pesto setup — answer a few questions to create your config.");
    println!("Press Enter to accept the [default] shown in brackets.\n");

    if path.exists() {
        let overwrite = ask("Config already exists; overwrite it? (y/N)", "n")?;
        if !overwrite.eq_ignore_ascii_case("y") {
            println!("Aborted; existing config kept.");
            return Ok(());
        }
    }

    let host = ask_required("NNTP server hostname")?;
    let port = ask("Server port", "563")?;
    let ssl = ask("Use TLS? (Y/n)", "y")?;
    let ssl = !ssl.eq_ignore_ascii_case("n");
    let connections = ask("Parallel connections", "10")?;
    let username = ask("Username (blank for none)", "")?;
    let password = if username.is_empty() {
        String::new()
    } else {
        ask("Password", "")?
    };
    let groups = ask("Newsgroups (comma-separated)", "alt.binaries.test")?;
    let from = ask("From header (blank = random identity per run)", "")?;
    let par2 = ask("PAR2 recovery percentage (0 disables)", "10")?;

    let mut toml = String::new();
    toml.push_str("# pesto configuration — generated by `pesto --config`.\n\n");
    toml.push_str("[server]\n");
    toml.push_str(&format!("host = \"{}\"\n", esc(&host)));
    toml.push_str(&format!("port = {}\n", port.trim()));
    toml.push_str(&format!("ssl = {ssl}\n"));
    toml.push_str(&format!("connections = {}\n", connections.trim()));
    toml.push('\n');

    if !username.is_empty() {
        toml.push_str("[auth]\n");
        toml.push_str(&format!("username = \"{}\"\n", esc(&username)));
        toml.push_str(&format!("password = \"{}\"\n\n", esc(&password)));
    }

    toml.push_str("[posting]\n");
    let group_list = groups
        .split(',')
        .map(|g| format!("\"{}\"", esc(g.trim())))
        .collect::<Vec<_>>()
        .join(", ");
    toml.push_str(&format!("groups = [{group_list}]\n"));
    if from.is_empty() {
        toml.push_str("# from omitted: each run posts under a random identity.\n");
    } else {
        toml.push_str(&format!("from = \"{}\"\n", esc(&from)));
    }
    toml.push_str(&format!("par2 = {}\n", par2.trim()));

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating config directory `{}`", parent.display()))?;
    }
    std::fs::write(&path, toml)
        .with_context(|| format!("writing config file `{}`", path.display()))?;

    println!("\nWrote {}", path.display());
    println!("pesto will load it automatically. Post a file with: pesto <FILE>");
    Ok(())
}

/// Prompt for a line of input, returning `default` when the user enters
/// nothing.
fn ask(label: &str, default: &str) -> Result<String> {
    if default.is_empty() {
        print!("{label}: ");
    } else {
        print!("{label} [{default}]: ");
    }
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("reading from stdin")?;
    let line = line.trim().to_string();
    Ok(if line.is_empty() {
        default.to_string()
    } else {
        line
    })
}

/// Prompt repeatedly until the user supplies a non-empty value.
fn ask_required(label: &str) -> Result<String> {
    loop {
        let value = ask(label, "")?;
        if !value.is_empty() {
            return Ok(value);
        }
        println!("  (required — please enter a value)");
    }
}

/// Escape a string for embedding inside a TOML double-quoted value.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pesto::walk::InputFile;

    fn inputs(names: &[&str]) -> Vec<InputFile> {
        names
            .iter()
            .map(|n| InputFile {
                path: PathBuf::from(n),
                name: n.to_string(),
            })
            .collect()
    }

    #[test]
    fn upload_root_finds_a_single_shared_directory() {
        assert_eq!(
            upload_root(&inputs(&["Show/ep01.bin", "Show/extras/clip.bin"])),
            Some("Show".to_string())
        );
    }

    #[test]
    fn upload_root_is_none_for_loose_or_mixed_inputs() {
        assert_eq!(upload_root(&inputs(&["a.bin"])), None);
        assert_eq!(upload_root(&inputs(&["A/x.bin", "B/y.bin"])), None);
        assert_eq!(upload_root(&inputs(&["Show/ep01.bin", "loose.bin"])), None);
    }
}
