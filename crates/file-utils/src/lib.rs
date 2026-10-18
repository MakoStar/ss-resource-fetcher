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
        let target = Self::resolve_path(path)?;
        fs::write(&target, data)
    }

    pub fn write_text(content: &str, path: impl AsRef<Path>) -> io::Result<()> {
        let target = Self::resolve_path(path)?;
        fs::write(&target, content.as_bytes())
    }

    pub fn write_json<T: Serialize>(data: &T, path: impl AsRef<Path>) -> io::Result<()> {
        let target = Self::resolve_path(path)?;
        let json_str = Self::to_json_pretty(data)?;
        fs::write(&target, json_str.as_bytes())
    }

    pub fn write_json_compact<T: Serialize>(data: &T, path: impl AsRef<Path>) -> io::Result<()> {
        let target = Self::resolve_path(path)?;
        let json_str = serde_json::to_string(data).map_err(io::Error::other)?;
        fs::write(&target, json_str.as_bytes())
    }

    pub fn write_json_atomic<T: Serialize>(data: &T, path: impl AsRef<Path>) -> io::Result<()> {
        let target = Self::resolve_path(path)?;
        let tmp = target.with_extension("tmp");
        let json_str = Self::to_json_pretty(data)?;
        fs::write(&tmp, json_str.as_bytes())?;
        fs::rename(&tmp, &target)
    }

    pub fn append_text(content: &str, path: impl AsRef<Path>) -> io::Result<()> {
        let target = Self::resolve_path(path)?;
        let mut file = OpenOptions::new().create(true).append(true).open(&target)?;
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
        let content = fs::read_to_string(p)
            .with_context(|| format!("[read_json] cannot read file: {}", p.display()))?;

        serde_json::from_str::<T>(&content)
            .with_context(|| format!("[read_json] JSON decode error: {}", p.display()))
    }

    pub fn read_lines(path: impl AsRef<Path>) -> Option<Vec<String>> {
        let p: &Path = path.as_ref();
        let file = match File::open(p) {
            Ok(f) => f,
            Err(e) => {
                Self::log_read_error("read_lines", p, &e);
                return None;
            }
        };
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
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
        let mut result = Vec::new();
        Self::walk_files_recursive(dir.as_ref(), &mut result);
        result
    }

    pub fn copy_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
        let dst = Self::resolve_path(dst)?;
        fs::copy(src.as_ref(), &dst)?;
        Ok(())
    }

    pub fn move_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
        let dst = Self::resolve_path(dst)?;
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
        let actual = Self::hash_file::<Md5>(path)
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
        let path = path.as_ref().to_path_buf();
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
        let mut buf = Vec::with_capacity(256);
        let mut ser: serde_json::Serializer<&mut Vec<u8>, serde_json::ser::PrettyFormatter<'_>> =
            serde_json::Serializer::with_formatter(&mut buf, formatter);
        data.serialize(&mut ser).map_err(io::Error::other)?;
        String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    fn hash_file<D: Digest>(path: impl AsRef<Path>) -> Option<String> {
        let p: &Path = path.as_ref();
        let mut file = match File::open(p) {
            Ok(f) => f,
            Err(e) => {
                warn!("[hash] Cannot open {}: {}", p.display(), e);
                return None;
            }
        };

        let mut hasher = D::new();
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
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Serialize};

    use super::FileHandler;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Sample {
        name: String,
        count: u32,
    }

    fn sample() -> Sample {
        Sample {
            name: "stella".into(),
            count: 7,
        }
    }

    #[test]
    fn writes_and_reads_back_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deep/data.bin");

        FileHandler::write_bytes(b"binary payload", &path).unwrap();

        assert_eq!(
            FileHandler::read_bytes(&path),
            Some(b"binary payload".to_vec())
        );
    }

    #[test]
    fn write_bytes_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c/d/file.txt");

        FileHandler::write_bytes(b"x", &path).unwrap();

        assert!(FileHandler::is_file(&path));
    }

    #[test]
    fn writes_and_reads_back_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");

        FileHandler::write_text("hello\nworld", &path).unwrap();

        assert_eq!(
            FileHandler::read_text(&path).as_deref(),
            Some("hello\nworld")
        );
    }

    #[test]
    fn json_roundtrip_is_pretty_printed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.json");

        FileHandler::write_json(&sample(), &path).unwrap();

        let text = FileHandler::read_text(&path).unwrap();
        assert!(
            text.contains("    "),
            "pretty json should indent with 4 spaces"
        );
        assert_eq!(FileHandler::read_json::<Sample>(&path).unwrap(), sample());
    }

    #[test]
    fn json_compact_is_single_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("compact.json");

        FileHandler::write_json_compact(&sample(), &path).unwrap();

        let text = FileHandler::read_text(&path).unwrap();
        assert!(!text.contains('\n'), "compact json should be one line");
        assert_eq!(FileHandler::read_json::<Sample>(&path).unwrap(), sample());
    }

    #[test]
    fn json_atomic_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atomic.json");

        FileHandler::write_json_atomic(&sample(), &path).unwrap();

        assert!(FileHandler::is_file(&path));
        assert!(!FileHandler::exists(path.with_extension("tmp")));
        assert_eq!(FileHandler::read_json::<Sample>(&path).unwrap(), sample());
    }

    #[test]
    fn appends_text_and_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.txt");

        FileHandler::append_text("a", &path).unwrap();
        FileHandler::append_line("b", &path).unwrap();
        FileHandler::append_text("c", &path).unwrap();

        assert_eq!(FileHandler::read_text(&path).unwrap(), "ab\nc");
        assert_eq!(FileHandler::read_lines(&path).unwrap(), vec!["ab", "c"]);
    }

    #[test]
    fn reads_missing_file_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.txt");

        assert_eq!(FileHandler::read_bytes(&missing), None);
        assert_eq!(FileHandler::read_text(&missing), None);
        assert_eq!(FileHandler::read_lines(&missing), None);
        assert_eq!(FileHandler::file_size(&missing), None);
    }

    #[test]
    fn computes_md5_and_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hash.bin");
        FileHandler::write_bytes(b"abc", &path).unwrap();

        assert_eq!(
            FileHandler::compute_md5(&path).as_deref(),
            Some("900150983cd24fb0d6963f7d28e17f72")
        );
        assert_eq!(
            FileHandler::compute_sha256(&path).as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }

    #[test]
    fn md5_accepts_uppercase_expected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hash.bin");
        FileHandler::write_bytes(b"abc", &path).unwrap();

        FileHandler::verify_md5(&path, "900150983CD24FB0D6963F7D28E17F72").unwrap();
    }

    #[test]
    fn verify_md5_reports_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hash.bin");
        FileHandler::write_bytes(b"abc", &path).unwrap();

        let err = FileHandler::verify_md5(&path, &"0".repeat(32)).unwrap_err();

        assert!(err.to_string().contains("MD5 mismatch"), "{err}");
        assert!(
            err.to_string().contains("900150983cd24fb0d6963f7d28e17f72"),
            "{err}"
        );
    }

    #[test]
    fn verify_md5_fails_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();

        assert!(FileHandler::verify_md5(dir.path().join("nope"), &"0".repeat(32)).is_err());
    }

    #[test]
    fn verifies_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hash.bin");
        FileHandler::write_bytes(b"abc", &path).unwrap();

        assert!(FileHandler::verify_sha256(
            &path,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ));
        assert!(!FileHandler::verify_sha256(&path, &"0".repeat(64)));
    }

    #[test]
    fn hashes_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.bin");
        FileHandler::write_bytes(b"", &path).unwrap();

        assert_eq!(
            FileHandler::compute_md5(&path).as_deref(),
            Some("d41d8cd98f00b204e9800998ecf8427e")
        );
    }

    #[test]
    fn hashes_large_file_in_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        let data: Vec<u8> = (0..=255u8).cycle().take(100_000).collect();
        FileHandler::write_bytes(&data, &path).unwrap();

        let expected = {
            use md5::{Digest, Md5};
            hex::encode(Md5::digest(&data))
        };

        assert_eq!(
            FileHandler::compute_md5(&path).as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn reports_size_of_written_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sized.bin");
        FileHandler::write_bytes(b"12345", &path).unwrap();

        assert_eq!(FileHandler::file_size(&path), Some(5));
    }

    #[test]
    fn copies_and_moves_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.bin");
        let dst = dir.path().join("out/copy.bin");
        FileHandler::write_bytes(b"payload", &src).unwrap();

        FileHandler::copy_file(&src, &dst).unwrap();
        assert_eq!(
            FileHandler::read_bytes(&dst).as_deref(),
            Some(&b"payload"[..])
        );

        let moved = dir.path().join("out/moved.bin");
        FileHandler::move_file(&dst, &moved).unwrap();
        assert!(!FileHandler::exists(&dst));
        assert_eq!(
            FileHandler::read_bytes(&moved).as_deref(),
            Some(&b"payload"[..])
        );
    }

    #[test]
    fn removes_file_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gone.txt");
        FileHandler::write_bytes(b"x", &path).unwrap();

        FileHandler::remove_file(&path).unwrap();
        assert!(!FileHandler::exists(&path));

        let sub = dir.path().join("sub");
        FileHandler::write_bytes(b"x", sub.join("inner.txt")).unwrap();
        FileHandler::remove_dir_all(&sub).unwrap();
        assert!(!FileHandler::exists(&sub));
    }

    #[test]
    fn lists_dir_and_walks_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        FileHandler::write_bytes(b"x", root.join("top.txt")).unwrap();
        FileHandler::write_bytes(b"x", root.join("a/mid.txt")).unwrap();
        FileHandler::write_bytes(b"x", root.join("a/b/deep.txt")).unwrap();

        let listed = FileHandler::list_dir(root).unwrap();
        assert_eq!(listed.len(), 2, "only direct children");

        let walked = FileHandler::walk_files(root);
        assert_eq!(walked.len(), 3, "recursive walk should find all files");
    }

    #[test]
    fn exists_is_file_is_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("thing.txt");
        FileHandler::write_bytes(b"x", &path).unwrap();

        assert!(FileHandler::exists(&path));
        assert!(FileHandler::is_file(&path));
        assert!(!FileHandler::is_dir(&path));
        assert!(FileHandler::is_dir(dir.path()));
        assert!(!FileHandler::exists(dir.path().join("nope")));
    }

    #[test]
    fn resolve_path_keeps_path_and_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x/y/z.txt");

        let resolved = FileHandler::resolve_path(&path).unwrap();

        assert_eq!(resolved, path);
        assert!(FileHandler::is_dir(dir.path().join("x/y")));
    }

    #[test]
    fn resolve_path_handles_bare_filename() {
        let resolved = FileHandler::resolve_path("bare.txt").unwrap();

        assert_eq!(resolved, std::path::PathBuf::from("bare.txt"));
    }

    #[test]
    fn ensure_dir_creates_nested_tree() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p/q/r");

        FileHandler::ensure_dir(&path).unwrap();
        FileHandler::ensure_dir(&path).unwrap();

        assert!(FileHandler::is_dir(&path));
    }

    #[test]
    fn json_handles_map_key_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("map.json");
        let mut map = BTreeMap::new();
        map.insert("zeta".to_string(), 1);
        map.insert("alpha".to_string(), 2);

        FileHandler::write_json(&map, &path).unwrap();

        let back: BTreeMap<String, i32> = FileHandler::read_json(&path).unwrap();
        assert_eq!(back, map);
    }
}
