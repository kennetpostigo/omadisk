use crate::protocol::{count_slice_nodes, is_other_path, other_path, OTHER_NAME};
use crate::walk::{Node, Tree};
use serde_json::{json, Value};
use std::collections::HashMap;

pub const DEFAULT_DEPTH: u32 = 3;
pub const DEFAULT_LIST_LIMIT: usize = 200;
pub const DEFAULT_SLICE_MIN_RATIO: f64 = 0.012;
pub const DEFAULT_MAX_SLICES: usize = 40;
pub const DEFAULT_MAX_FLAT: usize = 120;
pub const MAX_VIEW_BYTES: usize = 65536;

#[derive(Clone, Debug)]
pub struct ViewRec {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub bytes: u64,
    pub apparent: u64,
    pub partial: bool,
    pub error: String,
    pub child_count: usize,
    pub children: Vec<ViewRec>,
    #[allow(dead_code)]
    pub link_to: String,
}

pub trait TreeAdapter {
    fn get(&self, path: &str) -> Option<ViewRec>;
}

pub struct MemoryTree {
    tree: Tree,
    index: HashMap<String, usize>,
}

impl MemoryTree {
    pub fn new(tree: Tree) -> Self {
        let mut index = HashMap::new();
        for (i, node) in tree.nodes.iter().enumerate() {
            index.insert(node.path.clone(), i);
        }
        Self { tree, index }
    }
}

impl TreeAdapter for MemoryTree {
    fn get(&self, path: &str) -> Option<ViewRec> {
        let &idx = self.index.get(path)?;
        Some(node_to_rec(&self.tree, idx))
    }
}

fn node_to_rec(tree: &Tree, idx: usize) -> ViewRec {
    let node = &tree.nodes[idx];
    let children = node
        .children
        .iter()
        .map(|&c| {
            let child = &tree.nodes[c];
            ViewRec {
                path: child.path.clone(),
                name: child.name.clone(),
                kind: child.kind.clone(),
                bytes: child.bytes,
                apparent: child.apparent,
                partial: child.partial,
                error: child.error.clone(),
                child_count: child.children.len(),
                children: Vec::new(),
                link_to: child.link_to.clone(),
            }
        })
        .collect();
    ViewRec {
        path: node.path.clone(),
        name: node.name.clone(),
        kind: node.kind.clone(),
        bytes: node.bytes,
        apparent: node.apparent,
        partial: node.partial,
        error: node.error.clone(),
        child_count: node.children.len(),
        children,
        link_to: node.link_to.clone(),
    }
}

pub struct CacheTree {
    nodes: Value,
}

impl CacheTree {
    pub fn new(tree: Value) -> Self {
        Self {
            nodes: tree.get("nodes").cloned().unwrap_or_else(|| json!({})),
        }
    }
}

pub fn os_basename(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let stripped = path.trim_end_matches('/');
    if stripped.is_empty() {
        return "/".into();
    }
    stripped.rsplit('/').next().unwrap_or(path).to_string()
}

pub fn os_dirname(path: &str) -> String {
    let stripped = path.trim_end_matches('/');
    if stripped.is_empty() || !stripped.contains('/') {
        return "/".into();
    }
    let parent = stripped.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    if parent.is_empty() {
        "/".into()
    } else {
        parent.to_string()
    }
}

pub fn normalize_abs_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(path.len());
    let mut last_slash = false;
    for c in path.chars() {
        if c == '/' {
            if !last_slash {
                out.push('/');
            }
            last_slash = true;
        } else {
            out.push(c);
            last_slash = false;
        }
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

pub fn remap_cached_path(requested: &str, meta_root: &str, meta_real: &str) -> String {
    let req = normalize_abs_path(requested);
    let stored = normalize_abs_path(meta_root);
    let real = normalize_abs_path(meta_real);
    if req.is_empty() {
        return stored;
    }
    if req == stored || (!real.is_empty() && req == real) {
        return stored;
    }
    if !real.is_empty() && real != "/" && req.starts_with(&(real.clone() + "/")) {
        if stored == "/" {
            return req[real.len()..].to_string();
        }
        return stored + &req[real.len()..];
    }
    if stored == "/" {
        return req;
    }
    if req.starts_with(&(stored.clone() + "/")) {
        return req;
    }
    req
}

fn json_int(v: &Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_i64().map(|n| n.max(0) as u64))
        .unwrap_or(0)
}

impl TreeAdapter for CacheTree {
    fn get(&self, path: &str) -> Option<ViewRec> {
        let nodes = self.nodes.as_object()?;
        if let Some(node) = nodes.get(path) {
            let mut children = Vec::new();
            if let Some(arr) = node.get("c").and_then(Value::as_array) {
                for child in arr {
                    let child_path = child
                        .get("p")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let kind = child
                        .get("k")
                        .and_then(Value::as_str)
                        .unwrap_or("file")
                        .to_string();
                    let sub = if kind == "dir" || kind == "mount" {
                        nodes.get(&child_path)
                    } else {
                        None
                    };
                    let bytes = json_int(child.get("b").unwrap_or(&Value::Null));
                    let apparent = sub.and_then(|s| s.get("a")).map(json_int).unwrap_or(bytes);
                    let error = sub
                        .and_then(|s| s.get("err"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let child_count = sub
                        .and_then(|s| s.get("c"))
                        .and_then(Value::as_array)
                        .map(|a| a.len())
                        .unwrap_or(0);
                    children.push(ViewRec {
                        path: child_path.clone(),
                        name: child
                            .get("n")
                            .and_then(Value::as_str)
                            .unwrap_or(&os_basename(&child_path))
                            .to_string(),
                        kind,
                        bytes,
                        apparent,
                        partial: false,
                        error,
                        child_count,
                        children: Vec::new(),
                        link_to: String::new(),
                    });
                }
            }
            return Some(ViewRec {
                path: path.to_string(),
                name: node
                    .get("n")
                    .and_then(Value::as_str)
                    .unwrap_or(&os_basename(path))
                    .to_string(),
                kind: node
                    .get("k")
                    .and_then(Value::as_str)
                    .unwrap_or("dir")
                    .to_string(),
                bytes: json_int(node.get("b").unwrap_or(&Value::Null)),
                apparent: json_int(node.get("a").unwrap_or(&Value::Null))
                    .max(json_int(node.get("b").unwrap_or(&Value::Null))),
                partial: false,
                error: node
                    .get("err")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                child_count: node
                    .get("c")
                    .and_then(Value::as_array)
                    .map(|a| a.len())
                    .unwrap_or(0),
                children,
                link_to: String::new(),
            });
        }
        let parent = os_dirname(path);
        if let Some(pnode) = nodes.get(&parent) {
            if let Some(arr) = pnode.get("c").and_then(Value::as_array) {
                for child in arr {
                    if child.get("p").and_then(Value::as_str) == Some(path) {
                        let bytes = json_int(child.get("b").unwrap_or(&Value::Null));
                        return Some(ViewRec {
                            path: path.to_string(),
                            name: child
                                .get("n")
                                .and_then(Value::as_str)
                                .unwrap_or(&os_basename(path))
                                .to_string(),
                            kind: child
                                .get("k")
                                .and_then(Value::as_str)
                                .unwrap_or("file")
                                .to_string(),
                            bytes,
                            apparent: bytes,
                            partial: false,
                            error: String::new(),
                            child_count: 0,
                            children: Vec::new(),
                            link_to: String::new(),
                        });
                    }
                }
            }
        }
        None
    }
}

pub fn collapse_children(
    parent_bytes: u64,
    children: &[ViewRec],
    parent_path: &str,
    ratio: f64,
    max_slices: usize,
) -> Vec<ViewRec> {
    let mut ordered = children.to_vec();
    ordered.sort_by_key(|b| std::cmp::Reverse(b.bytes));
    if ordered.is_empty() {
        return Vec::new();
    }
    let threshold = if parent_bytes > 0 {
        (parent_bytes as f64) * ratio
    } else {
        0.0
    };
    let mut keep: Vec<ViewRec> = ordered
        .iter()
        .filter(|c| (c.bytes as f64) >= threshold)
        .take(max_slices)
        .cloned()
        .collect();
    if keep.is_empty() {
        keep = ordered.iter().take(1).cloned().collect();
    }
    let keep_paths: std::collections::HashSet<(String, String)> = keep
        .iter()
        .map(|c| (c.path.clone(), c.name.clone()))
        .collect();
    let rest: Vec<&ViewRec> = ordered
        .iter()
        .filter(|c| !keep_paths.contains(&(c.path.clone(), c.name.clone())))
        .collect();
    let mut out: Vec<ViewRec> = keep
        .into_iter()
        .map(|c| ViewRec {
            children: Vec::new(),
            ..c
        })
        .collect();
    let other_bytes: u64 = rest.iter().map(|c| c.bytes).sum();
    if other_bytes > 0 {
        out.push(ViewRec {
            name: OTHER_NAME.to_string(),
            path: other_path(parent_path),
            kind: "other".into(),
            bytes: other_bytes,
            apparent: other_bytes,
            partial: rest.iter().any(|c| c.partial),
            error: String::new(),
            child_count: rest.len(),
            children: Vec::new(),
            link_to: String::new(),
        });
    }
    out
}

fn rec_to_slice(child: &ViewRec) -> Value {
    json!({
        "name": child.name,
        "path": child.path,
        "kind": child.kind,
        "bytes": child.bytes,
        "partial": child.partial,
        "error": child.error,
        "childCount": child.child_count,
        "children": [],
    })
}

fn rec_to_list(child: &ViewRec) -> Value {
    json!({
        "name": child.name,
        "path": child.path,
        "kind": child.kind,
        "bytes": child.bytes,
        "partial": child.partial,
        "error": child.error,
        "childCount": child.child_count,
    })
}

fn expand_nested(
    tree: &dyn TreeAdapter,
    children: &mut [Value],
    remaining: usize,
    depth_left: u32,
    ratio: f64,
    max_slices: usize,
) -> usize {
    if remaining == 0 || depth_left == 0 {
        return remaining;
    }
    let mut frontier: Vec<usize> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.get("kind").and_then(Value::as_str) == Some("dir")
                && !is_other_path(c.get("path").and_then(Value::as_str).unwrap_or(""))
        })
        .map(|(i, _)| i)
        .collect();
    frontier.sort_by(|&a, &b| {
        json_int(children[b].get("bytes").unwrap_or(&Value::Null))
            .cmp(&json_int(children[a].get("bytes").unwrap_or(&Value::Null)))
    });
    let mut remaining = remaining;
    let mut next_frontier_idx: Vec<(usize, usize)> = Vec::new();
    for idx in frontier {
        if remaining == 0 {
            break;
        }
        let path = children[idx]
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let bytes = json_int(children[idx].get("bytes").unwrap_or(&Value::Null));
        let rec = tree.get(&path);
        let raw = rec.map(|r| r.children).unwrap_or_default();
        let collapsed = collapse_children(bytes, &raw, &path, ratio, max_slices);
        if collapsed.is_empty() {
            continue;
        }
        if collapsed.len() > remaining {
            continue;
        }
        let collapsed_json: Vec<Value> = collapsed.iter().map(rec_to_slice).collect();
        let n = collapsed_json.len();
        children[idx]["children"] = Value::Array(collapsed_json);
        remaining -= n;
        if depth_left > 1 {
            next_frontier_idx.push((idx, n));
        }
    }
    if depth_left > 1 && remaining > 0 {
        for (idx, _) in next_frontier_idx {
            if remaining == 0 {
                break;
            }
            if let Some(arr) = children[idx]
                .get_mut("children")
                .and_then(Value::as_array_mut)
            {
                remaining = expand_nested(tree, arr, remaining, depth_left - 1, ratio, max_slices);
            }
        }
    }
    remaining
}

fn drop_deepest(node: &mut Value, ring: u32, max_ring: u32) {
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    if ring >= max_ring {
        for child in children.iter_mut() {
            child["children"] = json!([]);
        }
        return;
    }
    for child in children.iter_mut() {
        drop_deepest(child, ring + 1, max_ring);
    }
}

fn encoded_len(event: &Value) -> usize {
    serde_json::to_string(event)
        .map(|s| s.len())
        .unwrap_or(usize::MAX)
}

fn fit_json(mut event: Value) -> Value {
    if encoded_len(&event) <= MAX_VIEW_BYTES {
        return event;
    }
    for max_ring in [3, 2, 1] {
        drop_deepest(&mut event, 1, max_ring);
        if encoded_len(&event) <= MAX_VIEW_BYTES {
            return event;
        }
    }
    loop {
        let too_big = encoded_len(&event) > MAX_VIEW_BYTES;
        if !too_big {
            break;
        }
        let Some(list) = event.get_mut("list").and_then(Value::as_array_mut) else {
            break;
        };
        if list.is_empty() {
            break;
        }
        list.pop();
        let truncated = event
            .get("listTruncated")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        event["listTruncated"] = json!(truncated);
    }
    event
}

pub struct ViewOpts {
    pub depth: u32,
    pub list_limit: usize,
    pub slice_min_ratio: f64,
    pub max_slices: usize,
    pub max_flat: usize,
    pub files: u64,
    pub dirs: u64,
    pub partial: bool,
}

impl Default for ViewOpts {
    fn default() -> Self {
        Self {
            depth: DEFAULT_DEPTH,
            list_limit: DEFAULT_LIST_LIMIT,
            slice_min_ratio: DEFAULT_SLICE_MIN_RATIO,
            max_slices: DEFAULT_MAX_SLICES,
            max_flat: DEFAULT_MAX_FLAT,
            files: 0,
            dirs: 0,
            partial: false,
        }
    }
}

pub fn build_view(tree: &dyn TreeAdapter, path: &str, opts: &ViewOpts) -> Value {
    let rec = tree.get(path).unwrap_or_else(|| ViewRec {
        path: path.to_string(),
        name: os_basename(path),
        kind: "dir".into(),
        bytes: 0,
        apparent: 0,
        partial: false,
        error: String::new(),
        child_count: 0,
        children: Vec::new(),
        link_to: String::new(),
    });
    let mut raw_children = rec.children.clone();
    raw_children.sort_by_key(|b| std::cmp::Reverse(b.bytes));
    let list_all: Vec<ViewRec> = raw_children
        .iter()
        .filter(|c| c.kind != "other")
        .cloned()
        .collect();
    let truncated = list_all.len().saturating_sub(opts.list_limit);
    let list_rows: Vec<Value> = list_all
        .iter()
        .take(opts.list_limit)
        .map(rec_to_list)
        .collect();
    let mut collapsed = collapse_children(
        rec.bytes,
        &raw_children,
        if rec.path.is_empty() { path } else { &rec.path },
        opts.slice_min_ratio,
        opts.max_slices,
    );
    let mut collapsed_json: Vec<Value> = collapsed.iter().map(rec_to_slice).collect();
    let remaining = opts.max_flat.saturating_sub(collapsed_json.len());
    if opts.depth > 1 && remaining > 0 {
        expand_nested(
            tree,
            &mut collapsed_json,
            remaining,
            opts.depth - 1,
            opts.slice_min_ratio,
            opts.max_slices,
        );
    }
    let _ = &mut collapsed;
    let mut event = json!({
        "v": 1,
        "type": "view",
        "path": if rec.path.is_empty() { path.to_string() } else { rec.path.clone() },
        "name": if rec.name.is_empty() { os_basename(path) } else { rec.name.clone() },
        "bytes": rec.bytes,
        "apparent": rec.apparent,
        "partial": rec.partial || opts.partial,
        "files": opts.files,
        "dirs": opts.dirs,
        "listTruncated": truncated,
        "children": collapsed_json,
        "list": list_rows,
    });
    if !rec.error.is_empty() {
        event["error"] = json!(rec.error);
    }
    event = fit_json(event);
    if count_slice_nodes(&event) > opts.max_flat {
        drop_deepest(&mut event, 1, 1);
    }
    event
}

#[allow(dead_code)]
pub fn synthetic_grid(breadth: usize, depth: usize, leaf_bytes: u64) -> Tree {
    let mut tree = Tree::default();
    fn make(
        tree: &mut Tree,
        path: &str,
        name: &str,
        level: usize,
        breadth: usize,
        depth: usize,
        leaf_bytes: u64,
    ) -> usize {
        if level >= depth {
            let idx = tree.nodes.len();
            tree.nodes.push(Node {
                path: path.to_string(),
                name: name.to_string(),
                kind: "file".into(),
                bytes: leaf_bytes,
                apparent: leaf_bytes,
                mtime: 0,
                error: String::new(),
                partial: false,
                link_to: String::new(),
                children: Vec::new(),
                dev: 0,
                ino: 0,
            });
            return idx;
        }
        let idx = tree.nodes.len();
        tree.nodes.push(Node {
            path: path.to_string(),
            name: name.to_string(),
            kind: "dir".into(),
            bytes: 0,
            apparent: 0,
            mtime: 0,
            error: String::new(),
            partial: false,
            link_to: String::new(),
            children: Vec::new(),
            dev: 0,
            ino: 0,
        });
        let mut total = 0u64;
        let mut kids = Vec::new();
        for i in 0..breadth {
            let child_name = format!("n{i}");
            let child_path = format!("{path}/{child_name}");
            let child = make(
                tree,
                &child_path,
                &child_name,
                level + 1,
                breadth,
                depth,
                leaf_bytes,
            );
            total += tree.nodes[child].bytes;
            kids.push(child);
        }
        tree.nodes[idx].children = kids;
        tree.nodes[idx].bytes = total;
        tree.nodes[idx].apparent = total;
        idx
    }
    let root = make(&mut tree, "/grid", "grid", 0, breadth, depth, leaf_bytes);
    tree.root = root;
    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::OTHER_SUFFIX;
    use std::fs;

    fn dir_node(path: &str, name: &str, children: Vec<Node>, nbytes: Option<u64>) -> (Tree, usize) {
        let mut tree = Tree::default();
        let idx = tree.nodes.len();
        tree.nodes.push(Node {
            path: path.to_string(),
            name: name.to_string(),
            kind: "dir".into(),
            bytes: nbytes.unwrap_or(0),
            apparent: nbytes.unwrap_or(0),
            mtime: 0,
            error: String::new(),
            partial: false,
            link_to: String::new(),
            children: Vec::new(),
            dev: 0,
            ino: 0,
        });
        let mut ids = Vec::new();
        let mut total = 0u64;
        for child in children {
            total += child.bytes;
            let cidx = tree.nodes.len();
            tree.nodes.push(child);
            ids.push(cidx);
        }
        tree.nodes[idx].children = ids;
        if nbytes.is_none() {
            tree.nodes[idx].bytes = total;
            tree.nodes[idx].apparent = total;
        }
        tree.root = idx;
        (tree, idx)
    }

    fn file_node(path: &str, name: &str, nbytes: u64) -> Node {
        Node {
            path: path.to_string(),
            name: name.to_string(),
            kind: "file".into(),
            bytes: nbytes,
            apparent: nbytes,
            mtime: 0,
            error: String::new(),
            partial: false,
            link_to: String::new(),
            children: Vec::new(),
            dev: 0,
            ino: 0,
        }
    }

    fn recs_from(children: Vec<(&str, &str, &str, u64)>) -> Vec<ViewRec> {
        children
            .into_iter()
            .map(|(name, path, kind, bytes)| ViewRec {
                name: name.into(),
                path: path.into(),
                kind: kind.into(),
                bytes,
                apparent: bytes,
                partial: false,
                error: String::new(),
                child_count: 0,
                children: Vec::new(),
                link_to: String::new(),
            })
            .collect()
    }

    #[test]
    fn other_collapse_ratio() {
        let children = recs_from(vec![
            ("big", "/t/big", "dir", 900),
            ("tiny", "/t/tiny", "file", 10),
            ("dust", "/t/dust", "file", 5),
        ]);
        let out = collapse_children(1000, &children, "/t", 0.012, 40);
        let names: Vec<_> = out.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"big"));
        assert!(names.contains(&"Other"));
        let other = out.iter().find(|c| c.kind == "other").unwrap();
        assert_eq!(other.path, other_path("/t"));
        assert_eq!(other.path, format!("/t{OTHER_SUFFIX}"));
        assert_eq!(other.bytes, 15);
        assert_eq!(other.child_count, 2);
    }

    #[test]
    fn max_40_slices_per_ring() {
        let children: Vec<ViewRec> = (0..80)
            .map(|i| ViewRec {
                name: format!("c{i}"),
                path: format!("/t/c{i}"),
                kind: "file".into(),
                bytes: 1000,
                apparent: 1000,
                partial: false,
                error: String::new(),
                child_count: 0,
                children: Vec::new(),
                link_to: String::new(),
            })
            .collect();
        let out = collapse_children(80_000, &children, "/t", 0.0, 40);
        assert_eq!(out.len(), 41);
        assert_eq!(out.last().unwrap().kind, "other");
        assert_eq!(out.last().unwrap().child_count, 40);
    }

    #[test]
    fn sort_desc() {
        let children = recs_from(vec![
            ("a", "/t/a", "file", 10),
            ("b", "/t/b", "file", 30),
            ("c", "/t/c", "file", 20),
        ]);
        let out = collapse_children(60, &children, "/t", 0.0, 40);
        let names: Vec<_> = out.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["b", "c", "a"]);
    }

    #[test]
    fn list_vs_children_and_truncated() {
        let kids: Vec<Node> = (0..250)
            .map(|i| file_node(&format!("/t/f{i}"), &format!("f{i}"), 1000 - i as u64))
            .collect();
        let (tree, _) = dir_node("/t", "t", kids, None);
        let ev = build_view(
            &MemoryTree::new(tree),
            "/t",
            &ViewOpts {
                list_limit: 200,
                max_slices: 40,
                slice_min_ratio: 0.0,
                ..ViewOpts::default()
            },
        );
        assert_eq!(ev["list"].as_array().unwrap().len(), 200);
        assert_eq!(ev["listTruncated"], 50);
        assert!(ev["children"].as_array().unwrap().len() <= 41);
        assert!(ev["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["kind"] == "other"));
        assert!(ev["list"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["kind"] != "other"));
        assert!(ev["list"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| !c["path"].as_str().unwrap_or("").contains('\0')));
    }

    #[test]
    fn empty_dir() {
        let (tree, _) = dir_node("/empty", "empty", vec![], None);
        let ev = build_view(&MemoryTree::new(tree), "/empty", &ViewOpts::default());
        assert!(ev["children"].as_array().unwrap().is_empty());
        assert!(ev["list"].as_array().unwrap().is_empty());
        assert_eq!(ev["listTruncated"], 0);
        assert_eq!(ev["bytes"], 0);
    }

    #[test]
    fn grid_41x41x41_caps() {
        let tree = synthetic_grid(41, 3, 1000);
        let ev = build_view(
            &MemoryTree::new(tree),
            "/grid",
            &ViewOpts {
                depth: 3,
                max_slices: 40,
                max_flat: 120,
                slice_min_ratio: 0.012,
                ..ViewOpts::default()
            },
        );
        assert!(count_slice_nodes(&ev) <= 120);
        let payload = serde_json::to_string(&ev).unwrap();
        assert!(payload.len() <= 65536);
    }

    #[test]
    fn depth3_emits_three_nested_arrays() {
        let mut atree = Tree::default();
        fn add(tree: &mut Tree, path: &str, name: &str, kind: &str, bytes: u64) -> usize {
            let idx = tree.nodes.len();
            tree.nodes.push(Node {
                path: path.into(),
                name: name.into(),
                kind: kind.into(),
                bytes,
                apparent: bytes,
                mtime: 0,
                error: String::new(),
                partial: false,
                link_to: String::new(),
                children: Vec::new(),
                dev: 0,
                ino: 0,
            });
            idx
        }
        let r = add(&mut atree, "/r", "r", "dir", 100);
        let a = add(&mut atree, "/r/a", "a", "dir", 100);
        let b = add(&mut atree, "/r/a/b", "b", "dir", 100);
        let c = add(&mut atree, "/r/a/b/c", "c", "file", 100);
        atree.nodes[r].children = vec![a];
        atree.nodes[a].children = vec![b];
        atree.nodes[b].children = vec![c];
        atree.root = r;
        let ev = build_view(
            &MemoryTree::new(atree),
            "/r",
            &ViewOpts {
                depth: 3,
                slice_min_ratio: 0.0,
                ..ViewOpts::default()
            },
        );
        assert_eq!(ev["children"][0]["name"], "a");
        assert_eq!(ev["children"][0]["children"][0]["name"], "b");
        assert_eq!(ev["children"][0]["children"][0]["children"][0]["name"], "c");
    }

    #[test]
    fn other_paths_unique_and_omitted_from_list() {
        fn bush(tree: &mut Tree, prefix: &str) -> usize {
            let name = prefix.rsplit('/').next().unwrap();
            let idx = tree.nodes.len();
            tree.nodes.push(Node {
                path: prefix.into(),
                name: name.into(),
                kind: "dir".into(),
                bytes: 0,
                apparent: 0,
                mtime: 0,
                error: String::new(),
                partial: false,
                link_to: String::new(),
                children: Vec::new(),
                dev: 0,
                ino: 0,
            });
            let mut kids = Vec::new();
            let mut total = 0u64;
            let big = tree.nodes.len();
            tree.nodes.push(Node {
                path: format!("{prefix}/big"),
                name: "big".into(),
                kind: "file".into(),
                bytes: 10_000,
                apparent: 10_000,
                mtime: 0,
                error: String::new(),
                partial: false,
                link_to: String::new(),
                children: Vec::new(),
                dev: 0,
                ino: 0,
            });
            kids.push(big);
            total += 10_000;
            for i in 0..50 {
                let cidx = tree.nodes.len();
                tree.nodes.push(Node {
                    path: format!("{prefix}/f{i}"),
                    name: format!("f{i}"),
                    kind: "file".into(),
                    bytes: 10,
                    apparent: 10,
                    mtime: 0,
                    error: String::new(),
                    partial: false,
                    link_to: String::new(),
                    children: Vec::new(),
                    dev: 0,
                    ino: 0,
                });
                kids.push(cidx);
                total += 10;
            }
            tree.nodes[idx].children = kids;
            tree.nodes[idx].bytes = total;
            tree.nodes[idx].apparent = total;
            idx
        }
        let mut tree = Tree::default();
        let r = tree.nodes.len();
        tree.nodes.push(Node {
            path: "/r".into(),
            name: "r".into(),
            kind: "dir".into(),
            bytes: 0,
            apparent: 0,
            mtime: 0,
            error: String::new(),
            partial: false,
            link_to: String::new(),
            children: Vec::new(),
            dev: 0,
            ino: 0,
        });
        let a = bush(&mut tree, "/r/a");
        let b = bush(&mut tree, "/r/b");
        tree.nodes[r].children = vec![a, b];
        tree.nodes[r].bytes = tree.nodes[a].bytes + tree.nodes[b].bytes;
        tree.nodes[r].apparent = tree.nodes[r].bytes;
        tree.root = r;
        let ev = build_view(
            &MemoryTree::new(tree),
            "/r",
            &ViewOpts {
                depth: 2,
                slice_min_ratio: 0.05,
                max_slices: 5,
                ..ViewOpts::default()
            },
        );
        let mut paths = Vec::new();
        fn collect(node: &Value, paths: &mut Vec<String>) {
            for child in node
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
            {
                if child["kind"] == "other" {
                    let expected =
                        other_path(node.get("path").and_then(Value::as_str).unwrap_or("/r"));
                    assert_eq!(child["path"].as_str().unwrap(), expected);
                    paths.push(child["path"].as_str().unwrap().to_string());
                }
                collect(&child, paths);
            }
        }
        collect(&ev, &mut paths);
        assert!(!paths.is_empty());
        let mut uniq = paths.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(paths.len(), uniq.len());
        assert!(paths.iter().all(|p| p.ends_with(OTHER_SUFFIX)));
        assert!(ev["list"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["kind"] != "other"));
    }

    #[test]
    fn golden_view_depth3_shape() {
        let path = format!(
            "{}/tests/goldens/view-depth3.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let data: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(data["type"], "view");
        assert!(!data["children"].as_array().unwrap().is_empty());
        let ring2 = data["children"][0]["children"].as_array().unwrap();
        assert!(!ring2.is_empty());
        let ring3 = ring2[0]["children"].as_array().unwrap();
        assert!(!ring3.is_empty());
        assert!(count_slice_nodes(&data) <= 120);
    }

    #[test]
    fn normalize_strips_trailing_and_duplicate_slashes() {
        assert_eq!(normalize_abs_path("/var/"), "/var");
        assert_eq!(normalize_abs_path("/var//log/"), "/var/log");
        assert_eq!(normalize_abs_path("/"), "/");
    }

    #[test]
    fn remap_view_path_uses_meta_root() {
        assert_eq!(
            remap_cached_path("/home/x/", "/home/x", "/home/x"),
            "/home/x"
        );
        assert_eq!(
            remap_cached_path("/home/x/.cache", "/home/x", "/home/x"),
            "/home/x/.cache"
        );
        assert_eq!(
            remap_cached_path("/real/home/x/.cache", "/home/x", "/real/home/x"),
            "/home/x/.cache"
        );
        assert_eq!(remap_cached_path("/var", "/", "/"), "/var");
        assert_eq!(
            remap_cached_path("/home/x/missing", "/home/x", "/home/x"),
            "/home/x/missing"
        );
    }

    #[test]
    fn other_path_is_parent_nul_other() {
        assert_eq!(other_path("/home"), format!("/home{OTHER_SUFFIX}"));
        assert!(other_path("/a") != other_path("/b"));
        assert!(other_path("/a").ends_with("/\0other"));
    }

    #[test]
    fn file_child_not_in_nodes_still_lists() {
        let file = file_node("/t/f", "f", 42);
        let (tree, _) = dir_node("/t", "t", vec![file], None);
        let ev = build_view(&MemoryTree::new(tree), "/t", &ViewOpts::default());
        assert_eq!(ev["list"][0]["kind"], "file");
        assert_eq!(ev["list"][0]["bytes"], 42);
        assert_eq!(ev["children"][0]["kind"], "file");
    }
}
