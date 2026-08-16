use crate::walk::Tree;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const CACHE_VERSION: u64 = 1;
pub const SLOT_CAP: usize = 4;
pub const FILE_MODE: u32 = 0o600;
pub const DIR_MODE: u32 = 0o700;

pub fn default_cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("omadisk");
        }
    }
    home_dir().join(".cache").join("omadisk")
}

pub fn home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    PathBuf::from("/")
}

fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

pub fn cache_key(
    root_realpath: &str,
    metric: &str,
    stay_on_fs: bool,
    hardlink_aware: bool,
) -> String {
    let payload = format!(
        "{root_realpath}\n{metric}\n{}\n{}",
        py_bool(stay_on_fs),
        py_bool(hardlink_aware)
    );
    let digest = Sha256::digest(payload.as_bytes());
    hex_prefix(&digest, 16)
}

fn hex_prefix(bytes: &[u8], chars: usize) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let n = chars / 2;
    let mut out = String::with_capacity(chars);
    for &b in bytes.iter().take(n) {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn chmod(path: &Path, mode: u32) {
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

pub fn ensure_dir(cache_dir: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = cache_dir.as_ref().to_path_buf();
    fs::create_dir_all(&path)?;
    chmod(&path, DIR_MODE);
    Ok(path)
}

pub fn scan_dir(cache_dir: &Path, key: &str) -> PathBuf {
    cache_dir.join("scans").join(key)
}

pub fn sweep_tmps(cache_dir: &Path, process_start: SystemTime) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    let scans = cache_dir.join("scans");
    if !scans.is_dir() {
        return removed;
    }
    let Ok(key_dirs) = fs::read_dir(&scans) else {
        return removed;
    };
    for entry in key_dirs.flatten() {
        let key_dir = entry.path();
        if !key_dir.is_dir() {
            continue;
        }
        let meta_missing = !key_dir.join("meta.json").is_file();
        let Ok(children) = fs::read_dir(&key_dir) else {
            continue;
        };
        for child in children.flatten() {
            let tmp = child.path();
            let name = child.file_name();
            if !name.to_string_lossy().ends_with(".tmp") {
                continue;
            }
            let Ok(meta) = tmp.metadata() else {
                continue;
            };
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if (mtime < process_start || meta_missing) && fs::remove_file(&tmp).is_ok() {
                removed.push(tmp);
            }
        }
    }
    removed
}

fn read_meta(path: &Path) -> Option<serde_json::Value> {
    let data = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&data).ok()?;
    let map = value.as_object()?;
    let v = map
        .get("v")
        .and_then(|x| x.as_u64().or_else(|| x.as_i64().map(|n| n as u64)))?;
    if v != CACHE_VERSION {
        return None;
    }
    Some(value)
}

pub fn evict_slots(cache_dir: &Path, keep: usize) -> Vec<PathBuf> {
    let scans = cache_dir.join("scans");
    if !scans.is_dir() {
        return Vec::new();
    }
    let Ok(rd) = fs::read_dir(&scans) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if dirs.len() <= keep {
        return Vec::new();
    }
    dirs.sort_by(|a, b| {
        sort_key(a)
            .cmp(&sort_key(b))
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });
    let extra = dirs.len() - keep;
    let mut evicted = Vec::new();
    for directory in dirs.into_iter().take(extra) {
        if fs::remove_dir_all(&directory).is_ok() {
            evicted.push(directory);
        }
    }
    evicted
}

fn sort_key(directory: &Path) -> (u8, i64) {
    match read_meta(&directory.join("meta.json")) {
        None => (0, 0),
        Some(meta) => {
            let finished = meta
                .get("finishedAt")
                .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
                .unwrap_or(0);
            (1, finished)
        }
    }
}

pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        chmod(parent, DIR_MODE);
    }
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(FILE_MODE)
            .open(&tmp)?;
        file.write_all(data)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        chmod(path, FILE_MODE);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn publish_scan(
    cache_dir: &Path,
    key: &str,
    meta: &serde_json::Value,
    tree: &serde_json::Value,
) -> io::Result<PathBuf> {
    let directory = scan_dir(cache_dir, key);
    fs::create_dir_all(&directory)?;
    chmod(&directory, DIR_MODE);
    let tree_bytes = format!(
        "{}\n",
        serde_json::to_string(tree).unwrap_or_else(|_| "{}".into())
    );
    let meta_bytes = format!(
        "{}\n",
        serde_json::to_string_pretty(meta).unwrap_or_else(|_| "{}".into())
    );
    atomic_write(&directory.join("tree.json"), tree_bytes.as_bytes())?;
    atomic_write(&directory.join("meta.json"), meta_bytes.as_bytes())?;
    evict_slots(cache_dir, SLOT_CAP);
    Ok(directory)
}

pub fn load_scan(cache_dir: &Path, key: &str) -> Option<(serde_json::Value, serde_json::Value)> {
    let directory = scan_dir(cache_dir, key);
    if !directory.is_dir() {
        return None;
    }
    let meta_path = directory.join("meta.json");
    let tree_path = directory.join("tree.json");
    let meta = read_meta(&meta_path);
    if meta.is_none() || !tree_path.is_file() {
        let _ = fs::remove_dir_all(&directory);
        return None;
    }
    let tree = match fs::read_to_string(&tree_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        Some(v)
            if v.as_object().is_some()
                && v.get("v")
                    .and_then(|x| x.as_u64().or_else(|| x.as_i64().map(|n| n as u64)))
                    == Some(CACHE_VERSION) =>
        {
            v
        }
        _ => {
            let _ = fs::remove_dir_all(&directory);
            return None;
        }
    };
    Some((meta.unwrap(), tree))
}

pub fn serialize_tree(tree: &Tree) -> serde_json::Value {
    let mut nodes = serde_json::Map::new();
    fn visit(tree: &Tree, idx: usize, nodes: &mut serde_json::Map<String, serde_json::Value>) {
        let node = &tree.nodes[idx];
        if node.kind != "dir" && node.kind != "mount" {
            return;
        }
        let mut children = Vec::new();
        for &child_idx in &node.children {
            let child = &tree.nodes[child_idx];
            children.push(serde_json::json!({
                "p": child.path,
                "n": child.name,
                "k": child.kind,
                "b": child.bytes,
            }));
            if child.kind == "dir" || child.kind == "mount" {
                visit(tree, child_idx, nodes);
            }
        }
        nodes.insert(
            node.path.clone(),
            serde_json::json!({
                "n": node.name,
                "k": node.kind,
                "b": node.bytes,
                "a": node.apparent,
                "m": node.mtime,
                "err": node.error,
                "c": children,
            }),
        );
    }
    if !tree.nodes.is_empty() {
        visit(tree, tree.root, &mut nodes);
        let root = &tree.nodes[tree.root];
        serde_json::json!({"v": CACHE_VERSION, "root": root.path, "nodes": nodes})
    } else {
        serde_json::json!({"v": CACHE_VERSION, "root": "", "nodes": {}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{Duration, UNIX_EPOCH};

    fn temp_cache() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omadisk-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        ensure_dir(&dir).unwrap()
    }

    #[test]
    fn ensure_dir_mode() {
        let cache = temp_cache();
        let mode = fs::metadata(&cache).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let _ = fs::remove_dir_all(&cache);
    }

    #[test]
    fn atomic_replace() {
        let cache = temp_cache();
        let path = cache.join("scans").join("k").join("tree.json");
        atomic_write(&path, b"{\"v\":1}\n").unwrap();
        assert!(path.is_file());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        atomic_write(&path, b"{\"v\":1,\"ok\":true}\n").unwrap();
        let data: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(data["ok"], true);
        assert!(!PathBuf::from(format!("{}.tmp", path.display())).exists());
        let _ = fs::remove_dir_all(&cache);
    }

    #[test]
    fn corrupt_meta_is_miss() {
        let cache = temp_cache();
        let key = "deadbeefdeadbeef";
        let directory = cache.join("scans").join(key);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("tree.json"),
            "{\"v\":1,\"root\":\"/\",\"nodes\":{}}\n",
        )
        .unwrap();
        fs::write(directory.join("meta.json"), "not-json").unwrap();
        assert!(load_scan(&cache, key).is_none());
        assert!(!directory.exists());
        let _ = fs::remove_dir_all(&cache);
    }

    #[test]
    fn key_differs_by_metric() {
        let a = cache_key("/home/x", "allocated", true, true);
        let b = cache_key("/home/x", "apparent", true, true);
        let c = cache_key("/home/x", "allocated", false, true);
        let d = cache_key("/home/x", "allocated", true, false);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn slot_cap_evicts_missing_meta_then_oldest() {
        let cache = temp_cache();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        for (i, finished) in [Some(50), Some(10), Some(30), Some(40), None]
            .into_iter()
            .enumerate()
        {
            let key = format!("slot{i:02}aaaaaaaaaa");
            if let Some(finished) = finished {
                let meta = json!({"v": 1, "finishedAt": finished, "key": key});
                let tree = json!({"v": 1, "root": "/", "nodes": {}});
                publish_scan(&cache, &key, &meta, &tree).unwrap();
                let meta_path = cache.join("scans").join(&key).join("meta.json");
                if meta_path.exists() {
                    let mut data: serde_json::Value =
                        serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
                    data["finishedAt"] = json!(finished);
                    fs::write(&meta_path, serde_json::to_string(&data).unwrap()).unwrap();
                }
            } else {
                let directory = cache.join("scans").join(&key);
                fs::create_dir_all(&directory).unwrap();
                fs::write(directory.join("tree.json"), "{}").unwrap();
            }
        }
        publish_scan(
            &cache,
            "slot99aaaaaaaaaa",
            &json!({"v": 1, "finishedAt": now, "key": "slot99aaaaaaaaaa"}),
            &json!({"v": 1, "root": "/", "nodes": {}}),
        )
        .unwrap();
        evict_slots(&cache, 4);
        let mut remaining: Vec<String> = fs::read_dir(cache.join("scans"))
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        remaining.sort();
        assert_eq!(remaining.len(), 4);
        assert!(!remaining.iter().any(|n| n == "slot04aaaaaaaaaa"));
        assert!(!remaining.iter().any(|n| n == "slot01aaaaaaaaaa"));
        let _ = fs::remove_dir_all(&cache);
    }

    #[test]
    fn startup_unlinks_stale_tmp() {
        let cache = temp_cache();
        let directory = cache.join("scans").join("abc");
        fs::create_dir_all(&directory).unwrap();
        let stale = directory.join("tree.json.tmp");
        fs::write(&stale, "partial").unwrap();
        let old = SystemTime::now() - Duration::from_secs(60);
        let _ = filetime_set(&stale, old);
        fs::write(directory.join("meta.json"), "{\"v\":1}\n").unwrap();
        let orphan_dir = cache.join("scans").join("orphan");
        fs::create_dir_all(&orphan_dir).unwrap();
        let orphan = orphan_dir.join("meta.json.tmp");
        fs::write(&orphan, "x").unwrap();
        let removed = sweep_tmps(&cache, SystemTime::now());
        assert!(!stale.exists());
        assert!(!orphan.exists());
        assert!(!removed.is_empty());
        let _ = fs::remove_dir_all(&cache);
    }

    fn filetime_set(path: &Path, when: SystemTime) -> io::Result<()> {
        let ts = filetime_from_system(when);
        let times = [ts, ts];
        let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn filetime_from_system(when: SystemTime) -> libc::timespec {
        let d = when.duration_since(UNIX_EPOCH).unwrap_or_default();
        libc::timespec {
            tv_sec: d.as_secs() as libc::time_t,
            tv_nsec: d.subsec_nanos() as libc::c_long,
        }
    }
}
