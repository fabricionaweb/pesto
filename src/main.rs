//! `pesto` — fast, lean Usenet poster.
//!
//! Phase 0 binary: parses the CLI, loads and resolves the configuration, and
//! reports the resolved settings. Posting itself arrives in later phases (see
//! ROADMAP.md).

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use pesto::config::{Config, FileConfig, Overrides};

/// Fast, lean Usenet poster: yEnc-encode files, post over NNTP, emit an .nzb.
#[derive(Parser, Debug)]
#[command(name = "pesto", version, about)]
struct Cli {
    /// Path to the TOML config file.
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// NNTP server hostname.
    #[arg(long)]
    host: Option<String>,

    /// NNTP server port.
    #[arg(long)]
    port: Option<u16>,

    /// Disable TLS (overrides the config file).
    #[arg(long)]
    no_ssl: bool,

    /// Number of parallel connections.
    #[arg(long)]
    connections: Option<usize>,

    /// Authentication username.
    #[arg(long)]
    username: Option<String>,

    /// Authentication password.
    #[arg(long)]
    password: Option<String>,

    /// `From` header used on posted articles.
    #[arg(long)]
    from: Option<String>,

    /// Newsgroups to post to (repeat or comma-separate).
    #[arg(long, value_delimiter = ',')]
    groups: Vec<String>,

    /// Path of the `.nzb` file to write.
    #[arg(short, long, value_name = "PATH")]
    out: Option<PathBuf>,

    /// Files to post.
    #[arg(required = true, value_name = "FILE")]
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
            article_size: None,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_config = match &cli.config {
        Some(path) => FileConfig::load(path)?,
        None => FileConfig::default(),
    };

    let config = Config::resolve(file_config, cli.overrides())?;

    let outcome = pesto::poster::post_files(&config, &cli.files).await?;

    println!("posted {} segment(s)", outcome.segments.len());
    if !outcome.failures.is_empty() {
        eprintln!("{} segment(s) failed:", outcome.failures.len());
        for failure in &outcome.failures {
            eprintln!("  - {failure}");
        }
    }

    // `.nzb` generation lands in ROADMAP.md phase 4.
    if let Some(out) = &cli.out {
        println!("nzb output ({}) — pending phase 4", out.display());
    }

    if !outcome.failures.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
