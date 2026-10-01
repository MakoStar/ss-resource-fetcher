use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use log::{error, warn};
use md5::{Digest, Md5};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::Sha256;

pub struct FileHandler;

impl FileHandler {
    pub fn write_bytes(data: &[u8], path: impl AsRef<Path>) -> io::Result<()> {
        let target: PathBuf = Self::resolve_path(path)?;
        fs::write(&target, data)
    }

    pub fn write_text(content: &str, path: impl AsRef<Path>) -> io::Result<()> {
        let target: PathBuf = Self::resolve_path(path)?;
        fs::write(&target, content.as_bytes())
    }

    pub fn write_json<T: Serialize>(data: &T, path: impl AsRef<Path>) -> io::Result<()> {
        let target: PathBuf = Self::resolve_path(path)?;
        let json_str: String = Self::to_json_pretty(data)?;
        fs::write(&target, json_str.as_bytes())
    }

    pub fn write_json_compact<T: Serialize>(data: &T, path: impl AsRef<Path>) -> io::Result<()> {
        let target: PathBuf = Self::resolve_path(path)?;
        let json_str: String = serde_json::to_string(data).map_err(|e| io::Error::other(e))?;
        fs::write(&target, json_str.as_bytes())
    }

    pub fn write_json_atomic<T: Serialize>(data: &T, path: impl AsRef<Path>) -> io::Result<()> {
        let target: PathBuf = Self::resolve_path(path)?;
        let tmp: PathBuf = target.with_extension("tmp");
        let json_str: String = Self::to_json_pretty(data)?;
        fs::write(&tmp, json_str.as_bytes())?;
        fs::rename(&tmp, &target)
    }

    pub fn append_text(content: &str, path: impl AsRef<Path>) -> io::Result<()> {
        let target: PathBuf = Self::resolve_path(path)?;
        let mut file: File = OpenOptions::new().create(true).append(true).open(&target)?;
        file.write_all(content.as_bytes())
    }

    pub fn append_line(line: &str, path: impl AsRef<Path>) -> io::Result<()> {
        Self::append_text(&format!("{}\n", line), path)
    }

    pub fn read_bytes(path: impl AsRef<Path>) -> Option<Vec<u8>> {
        let p: &Path = path.as_ref();
        match fs::read(p) {
            Ok(data) => Some(data),
            Err(e) => {
                Self::log_read_error("read_bytes", p, &e);
                None
            }
        }
    }

    pub fn read_text(path: impl AsRef<Path>) -> Option<String> {
        let p: &Path = path.as_ref();
        match fs::read_to_string(p) {
            Ok(content) => Some(content),
            Err(e) => {
                Self::log_read_error("read_text", p, &e);
                None
            }
        }
    }

    pub fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
        let p: &Path = path.as_ref();
        let content: String = fs::read_to_string(p)
            .with_context(|| format!("[read_json] cannot read file: {}", p.display()))?;

        serde_json::from_str::<T>(&content)
            .with_context(|| format!("[read_json] JSON decode error: {}", p.display()))
    }

    pub fn read_lines(path: impl AsRef<Path>) -> Option<Vec<String>> {
        let p: &Path = path.as_ref();
        let file: File = match File::open(p) {
            Ok(f) => f,
            Err(e) => {
                Self::log_read_error("read_lines", p, &e);
                return None;
            }
        };
        let reader: BufReader<File> = BufReader::new(file);
        let mut lines: Vec<String> = Vec::new();
        for line in reader.lines() {
            match line {
                Ok(l) => lines.push(l),
                Err(e) => {
                    warn!(
                        "[read_lines] Error reading line: {} | path: {}",
                        e,
                        p.display()
                    );
                    return None;
                }
            }
        }
        Some(lines)
    }

    pub fn ensure_dir(path: impl AsRef<Path>) -> io::Result<()> {
        fs::create_dir_all(path.as_ref())
    }

    pub fn exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }

    pub fn is_file(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_file()
    }

    pub fn is_dir(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_dir()
    }

    pub fn file_size(path: impl AsRef<Path>) -> Option<u64> {
        match fs::metadata(path.as_ref()) {
            Ok(meta) => Some(meta.len()),
            Err(_) => None,
        }
    }

    pub fn list_dir(dir: impl AsRef<Path>) -> Option<Vec<PathBuf>> {
        let p: &Path = dir.as_ref();
        match fs::read_dir(p) {
            Ok(entries) => {
                let mut paths = Vec::new();
                for entry in entries.flatten() {
                    paths.push(entry.path());
                }
                Some(paths)
            }
            Err(e) => {
                warn!(
                    "[list_dir] Failed to read dir: {} | path: {}",
                    e,
                    p.display()
                );
                None
            }
        }
    }

    pub fn walk_files(dir: impl AsRef<Path>) -> Vec<PathBuf> {
        let mut result: Vec<PathBuf> = Vec::new();
        Self::walk_files_recursive(dir.as_ref(), &mut result);
        result
    }

    pub fn copy_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
        let dst: PathBuf = Self::resolve_path(dst)?;
        fs::copy(src.as_ref(), &dst)?;
        Ok(())
    }

    pub fn move_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
        let dst: PathBuf = Self::resolve_path(dst)?;
        fs::rename(src.as_ref(), &dst)
    }

    pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        fs::remove_file(path.as_ref())
    }

    pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        fs::remove_dir_all(path.as_ref())
    }

    pub fn compute_md5(path: impl AsRef<Path>) -> Option<String> {
        Self::hash_file::<Md5>(path)
    }

    pub fn compute_sha256(path: impl AsRef<Path>) -> Option<String> {
        Self::hash_file::<Sha256>(path)
    }

    pub fn verify_md5(path: impl AsRef<Path>, expected: &str) -> anyhow::Result<()> {
        let path: &Path = path.as_ref();
        let actual: String = Self::hash_file::<Md5>(path)
            .ok_or_else(|| anyhow!("failed to compute MD5 for {}", path.display()))?;

        if actual.eq_ignore_ascii_case(expected) {
            Ok(())
        } else {
            Err(anyhow!(
                "MD5 mismatch for {}: expected {}, got {}",
                path.display(),
                expected,
                actual
            ))
        }
    }

    pub fn verify_sha256(path: impl AsRef<Path>, expected: &str) -> bool {
        Self::verify_hash_generic::<Sha256>(path, expected)
    }

    pub fn resolve_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let path: PathBuf = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        Ok(path)
    }

    fn to_json_pretty<T: Serialize>(data: &T) -> io::Result<String> {
        let formatter: serde_json::ser::PrettyFormatter<'_> =
            serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut buf: Vec<u8> = Vec::with_capacity(256);
        let mut ser: serde_json::Serializer<&mut Vec<u8>, serde_json::ser::PrettyFormatter<'_>> =
            serde_json::Serializer::with_formatter(&mut buf, formatter);
        data.serialize(&mut ser).map_err(|e| io::Error::other(e))?;
        String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    fn hash_file<D: Digest>(path: impl AsRef<Path>) -> Option<String> {
        let p: &Path = path.as_ref();
        let mut file: File = match File::open(p) {
            Ok(f) => f,
            Err(e) => {
                warn!("[hash] Cannot open {}: {}", p.display(), e);
                return None;
            }
        };

        let mut hasher: D = D::new();
        let mut buf: [u8; 8192] = [0u8; 8192];

        loop {
            match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buf[..n]),
                Err(e) => {
                    warn!("[hash] Read error {}: {}", p.display(), e);
                    return None;
                }
            }
        }
        Some(hex::encode(hasher.finalize()))
    }

    fn verify_hash_generic<D: Digest>(path: impl AsRef<Path>, expected: &str) -> bool {
        match Self::hash_file::<D>(path) {
            Some(actual) => actual.eq_ignore_ascii_case(expected),
            None => false,
        }
    }

    fn walk_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries: fs::ReadDir = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            if path.is_dir() {
                Self::walk_files_recursive(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    fn log_read_error(func: &str, path: &Path, err: &io::Error) {
        match err.kind() {
            io::ErrorKind::NotFound => {
                warn!("[{}] File not found: {}", func, path.display());
            }
            io::ErrorKind::PermissionDenied => {
                error!("[{}] Permission denied: {}", func, path.display());
            }
            _ => {
                error!("[{}] Read error: {} | path: {}", func, err, path.display());
            }
        }
    }
}
