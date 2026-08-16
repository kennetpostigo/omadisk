use serde_json::Value;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_omadisk-scan"))
}

fn write_exact(path: &Path, size: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, vec![b'x'; size]).unwrap();
}

fn make_tiny_tree(base: &Path) -> PathBuf {
    fs::create_dir_all(base).unwrap();
    let a = base.join("a");
    fs::create_dir_all(&a).unwrap();
    for i in 0..3 {
        write_exact(&a.join(format!("f{i}")), 4096);
    }
    let nested = base.join("b").join("c").join("d");
    fs::create_dir_all(&nested).unwrap();
    write_exact(&nested.join("file"), 2048);
    fs::create_dir_all(base.join("empty")).unwrap();
    write_exact(&base.join("big"), 1024 * 1024);
    fs::hard_link(base.join("big"), base.join("hard")).unwrap();
    symlink("a", base.join("linkdir")).unwrap();
    symlink("big", base.join("linkfile")).unwrap();
    base.to_path_buf()
}

fn unique_tmp(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse_line(line: &str) -> Option<Value> {
    let text = line.trim();
    if text.is_empty() {
        return None;
    }
    let obj: Value = serde_json::from_str(text).ok()?;
    if obj.get("v").and_then(Value::as_u64) != Some(1)
        && obj.get("v").and_then(Value::as_i64) != Some(1)
    {
        panic!("unsupported protocol version");
    }
    Some(obj)
}

struct Probe {
    tmp: PathBuf,
    root: PathBuf,
    cache: PathBuf,
}

impl Probe {
    fn new() -> Self {
        let tmp = unique_tmp("omadisk-cli");
        let root = make_tiny_tree(&tmp.join("tiny"));
        let cache = tmp.join("cache");
        Self { tmp, root, cache }
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(bin())
            .arg("--cache-dir")
            .arg(&self.cache)
            .args(args)
            .output()
            .expect("run scanner")
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

#[test]
fn proto() {
    let p = Probe::new();
    let out = p.run(&["proto"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let line = String::from_utf8_lossy(&out.stdout);
    let ev = parse_line(line.lines().next().unwrap()).unwrap();
    assert_eq!(ev["type"], "proto");
    assert_eq!(ev["v"], 1);
    assert!(p.cache.is_dir());
}

#[test]
fn tiny_scan_ndjson_and_golden() {
    let p = Probe::new();
    let out = p.run(&[
        "scan",
        "--root",
        &p.root.to_string_lossy(),
        "--emit-view-ms",
        "0",
        "--progress-ms",
        "0",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut events = Vec::new();
    for line in stdout.lines() {
        events.push(parse_line(line).unwrap_or_else(|| panic!("{line}")));
    }
    let types: Vec<_> = events
        .iter()
        .map(|e| e["type"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(types.first().map(String::as_str), Some("hello"));
    assert_eq!(types.last().map(String::as_str), Some("done"));
    assert!(types.iter().any(|t| t == "view"));
    let last = events.iter().rev().find(|e| e["type"] == "view").unwrap();
    assert_eq!(last["path"], p.root.to_string_lossy().as_ref());
    assert_eq!(last["partial"], false);
    let names: Vec<_> = last["list"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.iter().any(|n| n == "big"));
    assert!(names.iter().any(|n| n == "a"));
    assert!(names.iter().any(|n| n == "hard"));
    assert!(!names.iter().any(|n| n == "Other"));
    let hard = last["list"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "hard")
        .unwrap();
    assert_eq!(hard["kind"], "hardlink");
    let linkdir = last["list"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "linkdir")
        .unwrap();
    assert_eq!(linkdir["kind"], "symlink");

    let mut recorded = Vec::new();
    for ev in &events {
        if ev["type"] == "progress" {
            continue;
        }
        let mut slim = serde_json::json!({"v": ev["v"], "type": ev["type"]});
        match ev["type"].as_str().unwrap() {
            "hello" => {
                slim["root"] = serde_json::json!("$ROOT");
                slim["metric"] = ev["metric"].clone();
                slim["stayOnFilesystem"] = ev["stayOnFilesystem"].clone();
                slim["hardlinkAware"] = ev["hardlinkAware"].clone();
            }
            "view" => {
                slim["path"] = serde_json::json!("$ROOT");
                slim["name"] = serde_json::json!("$NAME");
                slim["partial"] = ev["partial"].clone();
            }
            "done" => {
                slim["partial"] = ev["partial"].clone();
            }
            _ => {}
        }
        recorded.push(slim);
    }
    let golden = fs::read_to_string(format!(
        "{}/tests/goldens/tiny-scan.ndjson",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let expected: Vec<Value> = golden
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(recorded, expected);
}

#[test]
fn view_after_scan() {
    let p = Probe::new();
    let scan = p.run(&[
        "scan",
        "--root",
        &p.root.to_string_lossy(),
        "--emit-view-ms",
        "0",
    ]);
    assert_eq!(
        scan.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let view = p.run(&[
        "view",
        "--root",
        &p.root.to_string_lossy(),
        "--path",
        &p.root.to_string_lossy(),
    ]);
    assert_eq!(
        view.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&view.stderr)
    );
    let ev = parse_line(
        String::from_utf8_lossy(&view.stdout)
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ev["type"], "view");
    assert!(view.stdout.len() <= 65536);
}

#[test]
fn view_miss_is_exit_3() {
    let p = Probe::new();
    let out = p.run(&["view", "--root", &p.root.to_string_lossy()]);
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn view_trailing_slash_hits_cache() {
    let p = Probe::new();
    let scan = p.run(&[
        "scan",
        "--root",
        &p.root.to_string_lossy(),
        "--emit-view-ms",
        "0",
    ]);
    assert_eq!(
        scan.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let slashed = format!("{}/", p.root.to_string_lossy());
    let view = p.run(&["view", "--root", &slashed, "--path", &slashed]);
    assert_eq!(
        view.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&view.stderr)
    );
    let ev = parse_line(
        String::from_utf8_lossy(&view.stdout)
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ev["type"], "view");
    assert_eq!(ev["path"], p.root.to_string_lossy().as_ref());
    assert!(ev["list"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "big"));
}

#[test]
fn view_unknown_path_falls_back_to_root() {
    let p = Probe::new();
    let scan = p.run(&[
        "scan",
        "--root",
        &p.root.to_string_lossy(),
        "--emit-view-ms",
        "0",
    ]);
    assert_eq!(
        scan.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let missing = p.root.join("no-such-child");
    let view = p.run(&[
        "view",
        "--root",
        &p.root.to_string_lossy(),
        "--path",
        &missing.to_string_lossy(),
    ]);
    assert_eq!(
        view.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&view.stderr)
    );
    let ev = parse_line(
        String::from_utf8_lossy(&view.stdout)
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ev["type"], "view");
    assert_eq!(ev["path"], p.root.to_string_lossy().as_ref());
    assert!(ev["list"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "big"));
}

#[test]
fn proto_creates_session_0600() {
    let p = Probe::new();
    let out = p.run(&["proto"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let session = p.cache.join("session.json");
    assert!(session.is_file());
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&session).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn scan_missing_root_is_exit_2() {
    let p = Probe::new();
    let missing = p.tmp.join("missing");
    let out = p.run(&["scan", "--root", &missing.to_string_lossy()]);
    assert_eq!(out.status.code(), Some(2));
}
