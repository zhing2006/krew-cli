//! Input history persistence (plain text, one entry per line).

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::StorageError;

type Result<T> = std::result::Result<T, StorageError>;

/// Escape a history entry for single-line storage.
/// `\` → `\\`, `\n` → literal `\n` (two chars).
pub fn escape_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

/// Unescape a stored history line back to the original string.
pub fn unescape_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Load all history entries from the given file path.
/// Returns an empty vec if the file does not exist.
pub fn load_history(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            entries.push(unescape_line(&line));
        }
    }
    Ok(entries)
}

/// Write all entries to the history file (full rewrite, used for truncation).
pub fn save_history(path: &Path, entries: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    for entry in entries {
        writeln!(file, "{}", escape_line(entry))?;
    }
    Ok(())
}

/// Append a single entry to the history file.
pub fn append_history_entry(path: &Path, entry: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", escape_line(entry))?;
    Ok(())
}
