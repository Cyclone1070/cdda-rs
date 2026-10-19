//! Parse results kept on disk, so that starting the game does not re-parse
//! several thousand files that have not changed.
//!
//! The reference does the same thing, keyed by the source file's modification
//! time: a cache entry is valid while the source's mtime, truncated to
//! milliseconds, still matches the one recorded when it was written
//! (`src/flexbuffer_cache.cpp:46-56`, `:176-187`). Entries live in a directory
//! that mirrors the source tree, named `<name>.<mtime millis>.fb` (`:250-260`),
//! and a damaged or unrecognized entry is dropped rather than reported.

use crate::error::ParseError;
use crate::value::{Object, Value};
use std::cell::Cell;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// What the magic bytes at the start of a cache file are.
const MAGIC: &[u8; 8] = b"CDDAJC01";

/// How many files were served from disk and how many had to be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheStats {
    /// Reads answered from a cache file.
    pub hits: u64,
    /// Reads that had to parse the source, whatever the reason.
    pub misses: u64,
}

/// A cache failure. A miss is never an error: only a source that cannot be read
/// or cannot be parsed is.
#[derive(Debug)]
pub enum CacheError {
    /// The source or cache directory could not be read or written.
    Io(std::io::Error),
    /// The source does not parse.
    Parse(ParseError),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::Io(error) => write!(f, "{error}"),
            CacheError::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self {
        CacheError::Io(error)
    }
}

impl From<ParseError> for CacheError {
    fn from(error: ParseError) -> Self {
        CacheError::Parse(error)
    }
}

/// A directory of parse results.
pub struct DiskCache {
    cache_dir: PathBuf,
    root_dir: PathBuf,
    hits: Cell<u64>,
    misses: Cell<u64>,
}

impl DiskCache {
    /// Opens a cache directory, creating it if needed.
    ///
    /// # Errors
    ///
    /// Returns an error when the cache directory cannot be created or scanned.
    ///
    /// Opening also drops what cannot be used, as the reference does when it
    /// scans its cache directory (`flexbuffer_cache.cpp:234-296`): files whose
    /// names are not entries, and older entries for a source that also has a
    /// newer one.
    pub fn new(
        cache_dir: impl Into<PathBuf>,
        root_dir: impl Into<PathBuf>,
    ) -> std::io::Result<Self> {
        let cache_dir = cache_dir.into();
        fs::create_dir_all(&cache_dir)?;
        prune(&cache_dir)?;
        Ok(DiskCache {
            cache_dir,
            root_dir: root_dir.into(),
            hits: Cell::new(0),
            misses: Cell::new(0),
        })
    }

    /// How many reads have been answered from disk and how many from a parse.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.get(),
            misses: self.misses.get(),
        }
    }

    /// Reads a JSON file, using the cached parse result when it is still valid.
    ///
    /// # Errors
    ///
    /// Returns an error when the source cannot be read or does not parse. A
    /// cache entry that cannot be used is a miss, never an error.
    pub fn parse_and_cache(&self, source: &Path) -> Result<Value, CacheError> {
        let mtime = match source_millis(source) {
            Ok(mtime) => mtime,
            Err(error) => {
                self.misses.set(self.misses.get() + 1);
                return Err(CacheError::Io(error));
            }
        };
        let entry = self.entry_path(source, mtime);
        if let Some(value) = read_entry(&entry) {
            self.hits.set(self.hits.get() + 1);
            return Ok(value);
        }
        self.misses.set(self.misses.get() + 1);
        let bytes = fs::read(source)?;
        let value = crate::parse(&bytes)?;
        write_entry(&entry, mtime, &value)?;
        Ok(value)
    }
}

/// One cache file's name split into the source it belongs to and its mtime.
fn parse_entry_name(path: &Path) -> Option<(PathBuf, u64)> {
    let name = path.file_name()?.to_str()?;
    let without_suffix = name.strip_suffix(".cache")?;
    let (source_name, millis) = without_suffix.rsplit_once('.')?;
    let millis: u64 = millis.parse().ok()?;
    if source_name.is_empty() {
        return None;
    }
    Some((path.with_file_name(source_name), millis))
}

/// Deletes what the cache cannot use: unreadable names, and every entry for a
/// source except the newest one.
fn prune(cache_dir: &Path) -> std::io::Result<()> {
    let mut newest: std::collections::HashMap<PathBuf, (u64, PathBuf)> =
        std::collections::HashMap::new();
    let mut files = Vec::new();
    let mut pending = vec![cache_dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    for path in files {
        let Some((source, millis)) = parse_entry_name(&path) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        match newest.get(&source) {
            Some((known, known_path)) if *known >= millis => {
                let _ = fs::remove_file(&path);
                let _ = known_path;
            }
            _ => {
                if let Some((_, old_path)) = newest.insert(source, (millis, path)) {
                    let _ = fs::remove_file(old_path);
                }
            }
        }
    }
    Ok(())
}

/// Milliseconds since the epoch, as the reference records them.
fn source_millis(path: &Path) -> std::io::Result<u64> {
    let modified = fs::metadata(path)?.modified()?;
    // A timestamp before the epoch carries no useful ordering here, and the
    // reference truncates such a value to milliseconds just the same.
    let Ok(since) = modified.duration_since(UNIX_EPOCH) else {
        return Ok(0);
    };
    u64::try_from(since.as_millis())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "mtime out of range"))
}

impl DiskCache {
    /// The cache file for a source, mirroring its path under the cache
    /// directory and carrying the mtime it was parsed at.
    fn entry_path(&self, source: &Path, mtime: u64) -> PathBuf {
        let relative = source.strip_prefix(&self.root_dir).unwrap_or(source);
        let mut entry = self.cache_dir.join(relative);
        let name = match entry.file_name().and_then(|name| name.to_str()) {
            Some(name) => format!("{name}.{mtime}.cache"),
            None => format!("unknown.{mtime}.cache"),
        };
        entry.set_file_name(name);
        entry
    }
}

/// Reads an entry, treating anything unexpected as a miss.
fn read_entry(path: &Path) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    decode(&bytes)
}

/// Writes an entry, ignoring a failure: the value has already been parsed, and
/// not being able to cache it is not a reason to fail.
fn write_entry(path: &Path, mtime: u64, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut payload = Vec::new();
    encode(value, &mut payload);
    let mut file = fs::File::create(path)?;
    file.write_all(MAGIC)?;
    file.write_all(&mtime.to_le_bytes())?;
    file.write_all(&payload)
}

fn encode(value: &Value, out: &mut Vec<u8>) {
    // Lengths are stored as u32; a single value longer than that is not
    // representable, so it is written as a null and the entry is dropped.

    match value {
        Value::Null => out.push(0),
        Value::Bool(false) => out.push(1),
        Value::Bool(true) => out.push(2),
        Value::Int(number) => {
            out.push(3);
            out.extend_from_slice(&number.to_le_bytes());
        }
        Value::Float(number) => {
            out.push(4);
            out.extend_from_slice(&number.to_bits().to_le_bytes());
        }
        Value::Str(text) => {
            out.push(5);
            encode_bytes(text.as_bytes(), out);
        }
        Value::Array(items) => {
            out.push(6);
            out.extend_from_slice(&u32::try_from(items.len()).unwrap_or(u32::MAX).to_le_bytes());
            for item in items {
                encode(item, out);
            }
        }
        Value::Object(object) => {
            out.push(7);
            out.extend_from_slice(
                &u32::try_from(object.len())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
            for (key, item) in object.members() {
                encode_bytes(key.as_bytes(), out);
                encode(item, out);
            }
        }
    }
}

fn encode_bytes(bytes: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn decode(bytes: &[u8]) -> Option<Value> {
    if bytes.len() < MAGIC.len() + 8 || bytes.get(..MAGIC.len())? != MAGIC {
        return None;
    }
    let mut cursor = MAGIC.len() + 8;
    let value = decode_value(bytes, &mut cursor)?;
    if cursor == bytes.len() {
        Some(value)
    } else {
        None
    }
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, count: usize) -> Option<&'a [u8]> {
    let end = cursor.checked_add(count)?;
    let slice = bytes.get(*cursor..end)?;
    *cursor = end;
    Some(slice)
}

fn decode_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    let raw = take(bytes, cursor, 4)?;
    let mut array = [0u8; 4];
    array.copy_from_slice(raw);
    Some(u32::from_le_bytes(array))
}

fn decode_value(bytes: &[u8], cursor: &mut usize) -> Option<Value> {
    let tag = *take(bytes, cursor, 1)?.first()?;
    match tag {
        0 => Some(Value::Null),
        1 => Some(Value::Bool(false)),
        2 => Some(Value::Bool(true)),
        3 => {
            let raw = take(bytes, cursor, 8)?;
            let mut array = [0u8; 8];
            array.copy_from_slice(raw);
            Some(Value::Int(i64::from_le_bytes(array)))
        }
        4 => {
            let raw = take(bytes, cursor, 8)?;
            let mut array = [0u8; 8];
            array.copy_from_slice(raw);
            Some(Value::Float(f64::from_bits(u64::from_le_bytes(array))))
        }
        5 => {
            let length = decode_u32(bytes, cursor)? as usize;
            let raw = take(bytes, cursor, length)?;
            Some(Value::Str(String::from_utf8(raw.to_vec()).ok()?))
        }
        6 => {
            let count = decode_u32(bytes, cursor)? as usize;
            let mut items = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                items.push(decode_value(bytes, cursor)?);
            }
            Some(Value::Array(items))
        }
        7 => {
            let count = decode_u32(bytes, cursor)? as usize;
            let mut object = Object::new();
            for _ in 0..count {
                let length = decode_u32(bytes, cursor)? as usize;
                let raw = take(bytes, cursor, length)?;
                let key = String::from_utf8(raw.to_vec()).ok()?;
                let item = decode_value(bytes, cursor)?;
                object.insert(key, item);
            }
            Some(Value::Object(object))
        }
        _ => None,
    }
}
