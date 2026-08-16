use crate::protocol::stat_event;
use serde_json::Value;
use std::ffi::CString;
use std::path::Path;

pub fn stat_path(path: &str) -> Value {
    let target = if path.is_empty() {
        crate::cache::home_dir().to_string_lossy().into_owned()
    } else {
        path.to_string()
    };
    let c_path = match CString::new(target.as_bytes()) {
        Ok(c) => c,
        Err(_) => return stat_event(&target, "", 0, 0, 0, false),
    };
    let mut buf: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut buf) };
    if rc != 0 {
        return stat_event(&target, "", 0, 0, 0, false);
    }
    let frsize = buf.f_frsize as u64;
    let used = buf.f_blocks.saturating_sub(buf.f_bfree) as u64 * frsize;
    let free = buf.f_bavail as u64 * frsize;
    let total = buf.f_blocks as u64 * frsize;
    let fs_path = Path::new(&target)
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| target.clone());
    stat_event(&target, &fs_path, used, free, total, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_plus_free_le_total_root() {
        let ev = stat_path("/");
        assert_eq!(ev["ok"], true);
        let used = ev["used"].as_u64().unwrap();
        let free = ev["free"].as_u64().unwrap();
        let total = ev["total"].as_u64().unwrap();
        assert!(used <= total);
        assert!(free <= total);
        assert!(total >= used);
    }

    #[test]
    fn used_plus_free_le_total_home() {
        let home = crate::cache::home_dir().to_string_lossy().into_owned();
        let ev = stat_path(&home);
        assert_eq!(ev["ok"], true);
        let used = ev["used"].as_u64().unwrap();
        let free = ev["free"].as_u64().unwrap();
        let total = ev["total"].as_u64().unwrap();
        assert!(used + free <= total);
    }

    #[test]
    fn missing_path_ok_false() {
        let ev = stat_path("/no/such/omadisk/path/does-not-exist-xyz");
        assert_eq!(ev["ok"], false);
        assert_eq!(ev["used"], 0);
        assert_eq!(ev["type"], "stat");
        assert_eq!(ev["v"], 1);
    }

    #[test]
    fn json_line() {
        let ev = stat_path("/");
        let line = serde_json::to_string(&ev).unwrap();
        let again: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(again["type"], "stat");
    }
}
