//! NFO file generation.
//!
//! Generates a plain-text `.nfo` summary describing the upload:
//! - Single media file → `mediainfo` output for that file.
//! - Series directory (name contains SXX pattern) → `mediainfo` of first episode.
//! - Generic directory (courses, documents, etc.) → banner + stats + directory tree.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

// ... (rest of the file with improved run_mediainfo)

fn run_mediainfo(path: &Path) -> std::io::Result<String> {
    let output = match std::process::Command::new("mediainfo").arg(path).output() {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(std::io::Error::other(
                "mediainfo not found in PATH — falling back to directory listing for .nfo.\n\
                 Install with: apt install mediainfo (or brew install media-info)"
            ));
        }
        Err(e) => return Err(e),
    };

    if !output.status.success() {
        return Err(std::io::Error::other("mediainfo exited non-zero"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ... full file content updated similarly for better messages