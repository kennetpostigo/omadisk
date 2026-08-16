use crate::protocol::allocated_bytes;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

pub const ALWAYS_SKIP_PREFIXES: &[&str] = &["/proc", "/dev", "/sys", "/run"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Allocated,
    Apparent,
}

impl Metric {
    pub fn as_str(self) -> &'static str {
        match self {
            Metric::Allocated => "allocated",
            Metric::Apparent => "apparent",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "allocated" => Some(Metric::Allocated),
            "apparent" => Some(Metric::Apparent),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StatInfo {
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_file: bool,
    pub dev: u64,
    pub ino: u64,
    pub nlink: u64,
    pub size: u64,
    pub blocks: u64,
    pub mtime: i64,
}

pub type StatFn = Box<dyn Fn(&Path, bool) -> io::Result<StatInfo>>;
type EnterFn = Box<dyn FnMut(&Walker, usize, Option<usize>)>;
type SkipFn = Box<dyn FnMut(&str, &str)>;
type ErrorFn = Box<dyn FnMut(&str, &str, bool)>;
type DirtyFn = Box<dyn FnMut(&Walker)>;

#[derive(Clone, Debug)]
pub struct Node {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub bytes: u64,
    pub apparent: u64,
    pub mtime: i64,
    pub error: String,
    pub partial: bool,
    pub link_to: String,
    pub children: Vec<usize>,
    pub dev: u64,
    pub ino: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
}

impl Tree {
    #[allow(dead_code)]
    pub fn root_node(&self) -> &Node {
        &self.nodes[self.root]
    }
}

pub fn is_skipped_prefix(path: &str, extra: &[String]) -> bool {
    ALWAYS_SKIP_PREFIXES
        .iter()
        .copied()
        .chain(extra.iter().map(String::as_str))
        .any(|prefix| {
            if prefix.is_empty() {
                return false;
            }
            let trimmed = prefix.trim_end_matches('/');
            path == prefix || path == trimmed || path.starts_with(&format!("{trimmed}/"))
        })
}

pub fn name_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return path.to_string();
    }
    trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

pub fn default_stat(path: &Path, follow: bool) -> io::Result<StatInfo> {
    let md = if follow {
        fs::metadata(path)?
    } else {
        fs::symlink_metadata(path)?
    };
    Ok(stat_from_metadata(&md))
}

fn stat_from_metadata(md: &fs::Metadata) -> StatInfo {
    let ft = md.file_type();
    StatInfo {
        is_dir: ft.is_dir(),
        is_symlink: ft.is_symlink(),
        is_file: ft.is_file()
            || (!ft.is_dir()
                && !ft.is_symlink()
                && !ft.is_socket()
                && !ft.is_fifo()
                && !ft.is_char_device()
                && !ft.is_block_device()),
        dev: md.dev(),
        ino: md.ino(),
        nlink: md.nlink(),
        size: md.size(),
        blocks: md.blocks(),
        mtime: md.mtime(),
    }
}

fn skip_reason(err: &io::Error) -> &'static str {
    match err.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EPERM) => "permission",
        _ => "io",
    }
}

struct Frame {
    node_idx: usize,
    entries: Option<Vec<(String, PathBuf)>>,
    index: usize,
}

pub struct Walker {
    pub root: PathBuf,
    pub metric: Metric,
    pub stay_on_fs: bool,
    pub follow_dir_symlinks: bool,
    pub count_hardlinks: bool,
    pub ignore: Vec<String>,
    pub max_errors: u64,
    pub stat_fn: Option<StatFn>,
    pub on_enter: Option<EnterFn>,
    pub on_leave: Option<EnterFn>,
    pub on_skip: Option<SkipFn>,
    pub on_error: Option<ErrorFn>,
    pub on_dirty: Option<DirtyFn>,
    pub files: u64,
    pub dirs: u64,
    pub skipped: u64,
    pub errors: u64,
    pub aborted: bool,
    pub dirty: bool,
    pub current: String,
    pub tree: Tree,
    seen_inodes: HashSet<(u64, u64)>,
    hardlink_paths: HashMap<(u64, u64), String>,
    root_dev: Option<u64>,
    signaled: Option<Box<dyn Fn() -> bool>>,
}

impl Walker {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let current = root.to_string_lossy().into_owned();
        Self {
            root,
            metric: Metric::Allocated,
            stay_on_fs: true,
            follow_dir_symlinks: false,
            count_hardlinks: false,
            ignore: Vec::new(),
            max_errors: 10_000,
            stat_fn: None,
            on_enter: None,
            on_leave: None,
            on_skip: None,
            on_error: None,
            on_dirty: None,
            files: 0,
            dirs: 0,
            skipped: 0,
            errors: 0,
            aborted: false,
            dirty: false,
            current,
            tree: Tree::default(),
            seen_inodes: HashSet::new(),
            hardlink_paths: HashMap::new(),
            root_dev: None,
            signaled: None,
        }
    }

    pub fn set_signal_check(&mut self, f: impl Fn() -> bool + 'static) {
        self.signaled = Some(Box::new(f));
    }

    fn should_stop(&self) -> bool {
        self.signaled.as_ref().is_some_and(|f| f())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        if let Some(mut cb) = self.on_dirty.take() {
            cb(self);
            self.on_dirty = Some(cb);
        }
    }

    fn skip(&mut self, path: &str, reason: &str) {
        self.skipped += 1;
        if let Some(mut cb) = self.on_skip.take() {
            cb(path, reason);
            self.on_skip = Some(cb);
        }
    }

    fn err(&mut self, path: &str, message: &str, fatal: bool) {
        if let Some(mut cb) = self.on_error.take() {
            cb(path, message, fatal);
            self.on_error = Some(cb);
        }
    }

    fn stat(&self, path: &Path, follow: bool) -> io::Result<StatInfo> {
        if let Some(f) = &self.stat_fn {
            f(path, follow)
        } else {
            default_stat(path, follow)
        }
    }

    fn size_of(&self, st: &StatInfo) -> (u64, u64) {
        let alloc = allocated_bytes(st.blocks);
        let apparent = st.size;
        if self.metric == Metric::Apparent {
            (apparent, apparent)
        } else {
            (alloc, apparent)
        }
    }

    fn add_to_ancestors(&mut self, stack: &[Frame], nbytes: u64, apparent: u64) {
        for frame in stack {
            let node = &mut self.tree.nodes[frame.node_idx];
            node.bytes = node.bytes.saturating_add(nbytes);
            node.apparent = node.apparent.saturating_add(apparent);
        }
        self.mark_dirty();
    }

    fn push_node(&mut self, node: Node) -> usize {
        let idx = self.tree.nodes.len();
        self.tree.nodes.push(node);
        idx
    }

    fn fire_enter(&mut self, idx: usize, parent: Option<usize>) {
        if let Some(mut cb) = self.on_enter.take() {
            cb(self, idx, parent);
            self.on_enter = Some(cb);
        }
    }

    fn fire_leave(&mut self, idx: usize, parent: Option<usize>) {
        if let Some(mut cb) = self.on_leave.take() {
            cb(self, idx, parent);
            self.on_leave = Some(cb);
        }
    }

    pub fn walk(&mut self) -> io::Result<&Tree> {
        let root = self.root.to_string_lossy().into_owned();
        if is_skipped_prefix(&root, &self.ignore) {
            self.skip(&root, "ignored");
            let idx = self.push_node(Node {
                path: root.clone(),
                name: name_of(&root),
                kind: "dir".into(),
                bytes: 0,
                apparent: 0,
                mtime: 0,
                error: "ignored".into(),
                partial: false,
                link_to: String::new(),
                children: Vec::new(),
                dev: 0,
                ino: 0,
            });
            self.tree.root = idx;
            return Ok(&self.tree);
        }

        let st = match self.stat(self.root.as_path(), true) {
            Ok(st) => st,
            Err(exc) => {
                let reason = skip_reason(&exc);
                self.skip(&root, reason);
                return Err(exc);
            }
        };

        if !st.is_dir {
            self.skip(&root, "not-a-directory");
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a directory",
            ));
        }

        self.root_dev = Some(st.dev);
        let (alloc, apparent) = self.size_of(&st);
        let root_idx = self.push_node(Node {
            path: root.clone(),
            name: name_of(&root),
            kind: "dir".into(),
            bytes: alloc,
            apparent,
            mtime: st.mtime,
            error: String::new(),
            partial: true,
            link_to: String::new(),
            children: Vec::new(),
            dev: st.dev,
            ino: st.ino,
        });
        self.tree.root = root_idx;
        self.dirs += 1;
        self.mark_dirty();
        self.fire_enter(root_idx, None);

        let mut stack = vec![Frame {
            node_idx: root_idx,
            entries: None,
            index: 0,
        }];
        let mut dir_stack: HashSet<(u64, u64)> = HashSet::new();
        dir_stack.insert((st.dev, st.ino));

        while !stack.is_empty() {
            if self.should_stop() {
                self.aborted = true;
                break;
            }
            if self.errors >= self.max_errors {
                self.aborted = true;
                break;
            }
            let top = stack.len() - 1;
            let node_idx = stack[top].node_idx;
            self.current = self.tree.nodes[node_idx].path.clone();

            if stack[top].entries.is_none() {
                let path = PathBuf::from(&self.tree.nodes[node_idx].path);
                let scanned = match fs::read_dir(&path) {
                    Ok(rd) => {
                        let mut entries: Vec<(String, PathBuf)> = Vec::new();
                        for ent in rd {
                            match ent {
                                Ok(e) => entries
                                    .push((e.file_name().to_string_lossy().into_owned(), e.path())),
                                Err(exc) => {
                                    let reason = skip_reason(&exc);
                                    let p = self.tree.nodes[node_idx].path.clone();
                                    if reason == "io" {
                                        self.errors += 1;
                                        self.err(&p, &format!("Error: {exc}"), false);
                                    }
                                    self.skip(&p, reason);
                                }
                            }
                        }
                        entries.sort_by(|a, b| a.0.cmp(&b.0));
                        entries
                    }
                    Err(exc) => {
                        let reason = skip_reason(&exc);
                        self.tree.nodes[node_idx].error = reason.to_string();
                        if reason == "io" {
                            self.errors += 1;
                            let p = self.tree.nodes[node_idx].path.clone();
                            self.err(&p, &format!("Error: {exc}"), false);
                        }
                        let p = self.tree.nodes[node_idx].path.clone();
                        self.skip(&p, reason);
                        Vec::new()
                    }
                };
                stack[top].entries = Some(scanned);
                stack[top].index = 0;
                continue;
            }

            let len = stack[top].entries.as_ref().map(|e| e.len()).unwrap_or(0);
            if stack[top].index >= len {
                self.tree.nodes[node_idx].partial = false;
                let ident = (self.tree.nodes[node_idx].dev, self.tree.nodes[node_idx].ino);
                stack.pop();
                dir_stack.remove(&ident);
                let parent = stack.last().map(|f| f.node_idx);
                self.mark_dirty();
                self.fire_leave(node_idx, parent);
                continue;
            }

            let index = stack[top].index;
            let (name, path) = stack[top].entries.as_ref().unwrap()[index].clone();
            stack[top].index += 1;
            self.handle_entry(node_idx, &name, &path, &mut stack, &mut dir_stack);
        }

        if !self.tree.nodes.is_empty() {
            self.tree.nodes[root_idx].partial = false;
        }
        Ok(&self.tree)
    }

    fn handle_entry(
        &mut self,
        parent_idx: usize,
        name: &str,
        path: &Path,
        stack: &mut Vec<Frame>,
        dir_stack: &mut HashSet<(u64, u64)>,
    ) {
        let path_s = path.to_string_lossy().into_owned();
        self.current = path_s.clone();
        if is_skipped_prefix(&path_s, &self.ignore) {
            self.skip(&path_s, "ignored");
            return;
        }

        let mut st = match self.stat(path, false) {
            Ok(st) => st,
            Err(exc) => {
                let reason = skip_reason(&exc);
                if reason == "io" {
                    self.errors += 1;
                    self.err(&path_s, &format!("Error: {exc}"), false);
                }
                self.skip(&path_s, reason);
                return;
            }
        };

        let mut is_link = st.is_symlink;
        let mut is_dir = st.is_dir && !is_link;
        let mut alloc_apparent = self.size_of(&st);

        if is_link && self.follow_dir_symlinks {
            match self.stat(path, true) {
                Ok(followed) if followed.is_dir => {
                    is_dir = true;
                    st = followed;
                    alloc_apparent = self.size_of(&st);
                    is_link = false;
                }
                Err(exc) => {
                    self.skip(&path_s, skip_reason(&exc));
                    return;
                }
                _ => {}
            }
        }

        let (alloc, apparent) = alloc_apparent;

        if is_dir {
            if self.stay_on_fs && self.root_dev.is_some_and(|d| st.dev != d) {
                let child_idx = self.push_node(Node {
                    path: path_s.clone(),
                    name: name.to_string(),
                    kind: "mount".into(),
                    bytes: alloc,
                    apparent,
                    mtime: st.mtime,
                    error: String::new(),
                    partial: false,
                    link_to: String::new(),
                    children: Vec::new(),
                    dev: st.dev,
                    ino: st.ino,
                });
                self.tree.nodes[parent_idx].children.push(child_idx);
                self.add_to_ancestors(stack, alloc, apparent);
                self.skip(&path_s, "other-fs");
                return;
            }
            let ident = (st.dev, st.ino);
            if dir_stack.contains(&ident) {
                self.skip(&path_s, "cycle");
                return;
            }
            let child_idx = self.push_node(Node {
                path: path_s,
                name: name.to_string(),
                kind: "dir".into(),
                bytes: alloc,
                apparent,
                mtime: st.mtime,
                error: String::new(),
                partial: true,
                link_to: String::new(),
                children: Vec::new(),
                dev: st.dev,
                ino: st.ino,
            });
            self.tree.nodes[parent_idx].children.push(child_idx);
            self.add_to_ancestors(stack, alloc, apparent);
            self.dirs += 1;
            self.fire_enter(child_idx, Some(parent_idx));
            dir_stack.insert(ident);
            stack.push(Frame {
                node_idx: child_idx,
                entries: None,
                index: 0,
            });
            return;
        }

        let mut kind = "file".to_string();
        let mut link_to = String::new();
        let mut nbytes = alloc;
        let mut napparent = apparent;
        if is_link {
            kind = "symlink".into();
        }
        if st.nlink > 1 && !self.count_hardlinks && !is_link && st.is_file {
            let ident = (st.dev, st.ino);
            if let Some(first) = self.hardlink_paths.get(&ident) {
                kind = "hardlink".into();
                nbytes = 0;
                napparent = 0;
                link_to = first.clone();
            } else {
                self.seen_inodes.insert(ident);
                self.hardlink_paths.insert(ident, path_s.clone());
            }
        }

        let child_idx = self.push_node(Node {
            path: path_s,
            name: name.to_string(),
            kind,
            bytes: nbytes,
            apparent: napparent,
            mtime: st.mtime,
            error: String::new(),
            partial: false,
            link_to,
            children: Vec::new(),
            dev: st.dev,
            ino: st.ino,
        });
        self.tree.nodes[parent_idx].children.push(child_idx);
        self.files += 1;
        self.add_to_ancestors(stack, nbytes, napparent);
    }
}

#[allow(dead_code)]
pub fn allocated_path(path: &Path) -> u64 {
    fs::symlink_metadata(path)
        .map(|md| allocated_bytes(md.blocks()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

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

    fn child_by_name<'a>(tree: &'a Tree, node: &Node, name: &str) -> Option<&'a Node> {
        node.children
            .iter()
            .map(|&i| &tree.nodes[i])
            .find(|c| c.name == name)
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

    #[test]
    fn totals_match_allocated_blocks() {
        let tmp = unique_tmp("omadisk-walk");
        let root = make_tiny_tree(&tmp);
        let mut walker = Walker::new(&root);
        walker.walk().unwrap();
        let mut expected = allocated_path(&root);
        fn walk_dir(dir: &Path, expected: &mut u64) {
            let rd = match fs::read_dir(dir) {
                Ok(rd) => rd,
                Err(_) => return,
            };
            for ent in rd.flatten() {
                let path = ent.path();
                let md = match fs::symlink_metadata(&path) {
                    Ok(md) => md,
                    Err(_) => continue,
                };
                if md.file_type().is_symlink() {
                    *expected += allocated_bytes(md.blocks());
                    continue;
                }
                if md.is_dir() {
                    *expected += allocated_bytes(md.blocks());
                    walk_dir(&path, expected);
                    continue;
                }
                if md.nlink() > 1 && path.file_name().and_then(|n| n.to_str()) == Some("hard") {
                    continue;
                }
                *expected += allocated_bytes(md.blocks());
            }
        }
        walk_dir(&root, &mut expected);
        assert_eq!(walker.tree.root_node().bytes, expected);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hardlink_counted_once() {
        let tmp = unique_tmp("omadisk-walk");
        let root = make_tiny_tree(&tmp);
        let mut walker = Walker::new(&root);
        walker.walk().unwrap();
        let tree = &walker.tree;
        let root_n = tree.root_node();
        let hard = child_by_name(tree, root_n, "hard").unwrap();
        let big = child_by_name(tree, root_n, "big").unwrap();
        assert_eq!(hard.kind, "hardlink");
        assert_eq!(hard.bytes, 0);
        assert!(big.bytes > 0);
        assert_eq!(hard.link_to, root.join("big").to_string_lossy());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn dir_symlink_not_descended() {
        let tmp = unique_tmp("omadisk-walk");
        let root = make_tiny_tree(&tmp);
        let mut walker = Walker::new(&root);
        walker.walk().unwrap();
        let tree = &walker.tree;
        let root_n = tree.root_node();
        let linkdir = child_by_name(tree, root_n, "linkdir").unwrap();
        assert_eq!(linkdir.kind, "symlink");
        assert!(linkdir.children.is_empty());
        let a = child_by_name(tree, root_n, "a").unwrap();
        assert_eq!(a.kind, "dir");
        assert_eq!(a.children.len(), 3);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn stay_on_fs_mock_st_dev() {
        let tmp = unique_tmp("omadisk-walk");
        let root = make_tiny_tree(&tmp);
        let other = root.join("mnt");
        fs::create_dir(&other).unwrap();
        write_exact(&other.join("inside"), 4096);
        let other_s = other.clone();
        let mut walker = Walker::new(&root);
        walker.stay_on_fs = true;
        walker.stat_fn = Some(Box::new(move |path, follow| {
            let mut st = default_stat(path, follow)?;
            if path == other_s.as_path() || path.starts_with(&other_s) {
                st.dev = st.dev.saturating_add(99);
            }
            Ok(st)
        }));
        walker.walk().unwrap();
        let tree = &walker.tree;
        let mnt = child_by_name(tree, tree.root_node(), "mnt").unwrap();
        assert_eq!(mnt.kind, "mount");
        assert!(mnt.children.is_empty());
        assert_eq!(mnt.bytes, allocated_path(&other));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn skip_prefixes() {
        assert!(is_skipped_prefix("/proc", &[]));
        assert!(is_skipped_prefix("/proc/1", &[]));
        assert!(is_skipped_prefix("/dev/null", &[]));
        assert!(is_skipped_prefix("/sys/class", &[]));
        assert!(is_skipped_prefix("/run/user/1000", &[]));
        assert!(!is_skipped_prefix("/home/proc", &[]));
        let tmp = unique_tmp("omadisk-walk");
        let root = make_tiny_tree(&tmp);
        assert!(!is_skipped_prefix(
            &root.join("proc").to_string_lossy(),
            &[]
        ));
        let ignored = root.join("a").to_string_lossy().into_owned();
        let mut walker = Walker::new(&root);
        walker.ignore = vec![ignored];
        use std::cell::RefCell;
        use std::rc::Rc;
        let skips = Rc::new(RefCell::new(Vec::<(String, String)>::new()));
        let skips2 = skips.clone();
        walker.on_skip = Some(Box::new(move |path, reason| {
            skips2
                .borrow_mut()
                .push((path.to_string(), reason.to_string()));
        }));
        walker.walk().unwrap();
        let names: Vec<_> = walker
            .tree
            .root_node()
            .children
            .iter()
            .map(|&i| walker.tree.nodes[i].name.as_str())
            .collect();
        assert!(!names.contains(&"a"));
        assert!(skips.borrow().iter().any(|(_, r)| r == "ignored"));
        let _ = fs::remove_dir_all(&tmp);
        let _ = skips;
    }

    #[test]
    fn permission_skip() {
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let tmp = unique_tmp("omadisk-walk");
        let root = make_tiny_tree(&tmp);
        let secret = root.join("secret");
        fs::create_dir(&secret).unwrap();
        write_exact(&secret.join("hidden"), 512);
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o0)).unwrap();
        use std::cell::RefCell;
        use std::rc::Rc;
        let skips = Rc::new(RefCell::new(Vec::new()));
        let skips2 = skips.clone();
        let mut walker = Walker::new(&root);
        walker.on_skip = Some(Box::new(move |_p, r| {
            skips2.borrow_mut().push(r.to_string());
        }));
        let result = walker.walk();
        let _ = fs::set_permissions(&secret, fs::Permissions::from_mode(0o700));
        result.unwrap();
        let secret_node = child_by_name(&walker.tree, walker.tree.root_node(), "secret").unwrap();
        assert_eq!(secret_node.error, "permission");
        assert!(skips.borrow().iter().any(|r| r == "permission"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn attach_on_enter() {
        let tmp = unique_tmp("omadisk-walk");
        let root = make_tiny_tree(&tmp);
        use std::cell::RefCell;
        use std::rc::Rc;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen2 = seen.clone();
        let root_s = root.to_string_lossy().into_owned();
        let mut walker = Walker::new(&root);
        walker.on_enter = Some(Box::new(move |w, idx, parent| {
            let Some(parent) = parent else {
                return;
            };
            let child = &w.tree.nodes[idx];
            assert!(w.tree.nodes[parent].children.contains(&idx));
            assert!(child.partial);
            assert!(child.bytes >= allocated_path(Path::new(&child.path)));
            seen2.borrow_mut().push(child.path.clone());
            let _ = &root_s;
        }));
        walker.walk().unwrap();
        let seen = seen.borrow();
        assert!(seen.iter().any(|p| p == &root.join("a").to_string_lossy()));
        assert!(seen
            .iter()
            .any(|p| p == &root.join("b").join("c").join("d").to_string_lossy()));
        let _ = fs::remove_dir_all(&tmp);
    }
}
