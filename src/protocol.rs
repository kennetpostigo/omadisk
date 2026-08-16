use serde_json::{json, Value};

pub const PROTOCOL_VERSION: u64 = 1;
pub const OTHER_NAME: &str = "Other";
pub const OTHER_SUFFIX: &str = "/\0other";

#[allow(dead_code)]
pub const EVENT_TYPES: &[&str] = &[
    "hello", "progress", "skip", "error", "view", "done", "stat", "proto",
];
#[allow(dead_code)]
pub const SKIP_REASONS: &[&str] = &[
    "permission",
    "cycle",
    "ignored",
    "other-fs",
    "not-a-directory",
    "io",
];

pub fn other_path(parent_path: &str) -> String {
    format!("{parent_path}{OTHER_SUFFIX}")
}

pub fn is_other_path(path: &str) -> bool {
    path.ends_with(OTHER_SUFFIX)
}

pub fn emit_line(obj: &Value) -> String {
    serde_json::to_string(obj).unwrap_or_else(|_| "{}".to_string())
}

pub fn hello(
    pid: u32,
    root: &str,
    metric: &str,
    stay_on_filesystem: bool,
    hardlink_aware: bool,
    started_at: i64,
    cache_key: &str,
) -> Value {
    json!({
        "v": PROTOCOL_VERSION,
        "type": "hello",
        "pid": pid,
        "root": root,
        "metric": metric,
        "stayOnFilesystem": stay_on_filesystem,
        "hardlinkAware": hardlink_aware,
        "startedAt": started_at,
        "cacheKey": cache_key,
    })
}

pub fn progress(files: u64, dirs: u64, bytes_total: u64, current: &str, skipped: u64) -> Value {
    json!({
        "v": PROTOCOL_VERSION,
        "type": "progress",
        "files": files,
        "dirs": dirs,
        "bytes": bytes_total,
        "current": current,
        "skipped": skipped,
    })
}

pub fn skip(path: &str, reason: &str) -> Value {
    json!({
        "v": PROTOCOL_VERSION,
        "type": "skip",
        "path": path,
        "reason": reason,
    })
}

pub fn error(path: &str, message: &str, fatal: bool) -> Value {
    json!({
        "v": PROTOCOL_VERSION,
        "type": "error",
        "path": path,
        "message": message,
        "fatal": fatal,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn done(
    files: u64,
    dirs: u64,
    bytes_total: u64,
    elapsed_ms: u64,
    skipped: u64,
    errors: u64,
    cache_key: &str,
    partial: bool,
) -> Value {
    json!({
        "v": PROTOCOL_VERSION,
        "type": "done",
        "files": files,
        "dirs": dirs,
        "bytes": bytes_total,
        "elapsedMs": elapsed_ms,
        "skipped": skipped,
        "errors": errors,
        "cacheKey": cache_key,
        "partial": partial,
    })
}

pub fn proto_event(cache_dir: &str) -> Value {
    json!({
        "v": PROTOCOL_VERSION,
        "type": "proto",
        "protocol": PROTOCOL_VERSION,
        "cacheDir": cache_dir,
    })
}

pub fn stat_event(path: &str, fs_path: &str, used: u64, free: u64, total: u64, ok: bool) -> Value {
    json!({
        "v": PROTOCOL_VERSION,
        "type": "stat",
        "path": path,
        "fsPath": fs_path,
        "used": used,
        "free": free,
        "total": total,
        "ok": ok,
    })
}

#[allow(dead_code)]
pub fn parse_line(line: &str) -> Result<Option<Value>, String> {
    let text = line.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let obj: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let Some(map) = obj.as_object() else {
        return Ok(None);
    };
    if let Some(v) = map.get("v") {
        if v.as_u64() != Some(PROTOCOL_VERSION) && v.as_i64() != Some(PROTOCOL_VERSION as i64) {
            return Err(format!("unsupported protocol version: {v}"));
        }
    }
    Ok(Some(obj))
}

#[allow(dead_code)]
pub fn validate_event(obj: &Value) -> bool {
    let Some(map) = obj.as_object() else {
        return false;
    };
    if map.get("v").and_then(Value::as_u64) != Some(PROTOCOL_VERSION)
        && map.get("v").and_then(Value::as_i64) != Some(PROTOCOL_VERSION as i64)
    {
        return false;
    }
    let Some(typ) = map.get("type").and_then(Value::as_str) else {
        return false;
    };
    if !EVENT_TYPES.contains(&typ) {
        return false;
    }
    if typ == "skip" {
        let reason = map.get("reason").and_then(Value::as_str).unwrap_or("");
        if !SKIP_REASONS.contains(&reason) {
            return false;
        }
    }
    true
}

pub fn count_slice_nodes(view: &Value) -> usize {
    fn walk(node: &Value, count: &mut usize) {
        let Some(children) = node.get("children").and_then(Value::as_array) else {
            return;
        };
        for child in children {
            *count += 1;
            walk(child, count);
        }
    }
    let mut count = 0;
    walk(view, &mut count);
    count
}

pub fn allocated_bytes(blocks: u64) -> u64 {
    blocks.saturating_mul(512)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn constructors_emit_v_and_type() {
        let events = [
            hello(1, "/tmp", "allocated", true, true, 1, "abcd"),
            progress(1, 1, 2, "/tmp", 0),
            skip("/tmp/x", "permission"),
            error("/tmp", "boom", false),
            done(1, 1, 2, 3, 0, 0, "abcd", false),
            proto_event("/tmp/cache"),
            stat_event("/tmp", "/", 1, 2, 4, true),
        ];
        for ev in events {
            assert_eq!(ev["v"], PROTOCOL_VERSION);
            assert!(ev.get("type").is_some());
            assert!(validate_event(&ev));
        }
    }

    #[test]
    fn parse_line_ignores_unknown_types() {
        let ev = parse_line(r#"{"v":1,"type":"future","extra":true}"#)
            .unwrap()
            .unwrap();
        assert_eq!(ev["type"], "future");
        assert!(!validate_event(&ev));
    }

    #[test]
    fn parse_line_rejects_v2() {
        assert!(parse_line(r#"{"v":2,"type":"hello"}"#).is_err());
    }

    #[test]
    fn parse_line_empty_and_bad_json() {
        assert!(parse_line("").unwrap().is_none());
        assert!(parse_line("not-json").unwrap().is_none());
    }

    #[test]
    fn protocol_md_examples_parse() {
        let md = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("protocol.md");
        let text = fs::read_to_string(md).unwrap();
        let mut parsed = 0;
        let mut in_fence = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence && trimmed.starts_with('{') {
                let ev = parse_line(trimmed).unwrap();
                assert!(ev.is_some(), "{trimmed}");
                assert_eq!(ev.unwrap()["v"], 1);
                parsed += 1;
            }
        }
        assert!(parsed >= 4);
    }

    #[test]
    fn json_roundtrip() {
        let ev = skip("/home/x", "cycle");
        let again: Value = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(again["reason"], "cycle");
    }
}
