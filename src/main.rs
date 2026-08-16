mod cache;
mod protocol;
mod statfs;
mod view;
mod walk;

use cache::{
    cache_key, default_cache_dir, ensure_dir, load_scan, publish_scan, serialize_tree, sweep_tmps,
};
use protocol::{done, emit_line, error, hello, progress, proto_event, skip};
use std::cell::Cell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use view::{
    build_view, remap_cached_path, CacheTree, MemoryTree, TreeAdapter, ViewOpts, DEFAULT_DEPTH,
    DEFAULT_LIST_LIMIT, DEFAULT_MAX_FLAT, DEFAULT_MAX_SLICES, DEFAULT_SLICE_MIN_RATIO,
};
use walk::{Metric, Walker};

const EXIT_OK: i32 = 0;
const EXIT_USAGE: i32 = 1;
const EXIT_ROOT: i32 = 2;
const EXIT_CACHE: i32 = 3;
const EXIT_ABORT: i32 = 4;
const EXIT_SIGNAL: i32 = 130;

static SIGNALED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALED.store(true, Ordering::SeqCst);
}

fn signaled() -> bool {
    SIGNALED.load(Ordering::SeqCst)
}

fn install_signals() {
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
}

fn emit(obj: &serde_json::Value) -> io::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(out, "{}", emit_line(obj))?;
    out.flush()
}

fn log_err(msg: &str) {
    let _ = writeln!(io::stderr(), "omadisk-scan: {msg}");
}

fn home() -> String {
    cache::home_dir().to_string_lossy().into_owned()
}

fn is_abs(path: &str) -> bool {
    path.starts_with('/')
}

fn has_nul(path: &str) -> bool {
    path.contains('\0')
}

struct ScanArgs {
    root: Option<String>,
    metric: Metric,
    cross_fs: bool,
    follow_dir_symlinks: bool,
    count_hardlinks: bool,
    ignore: Vec<String>,
    emit_view_ms: u64,
    progress_ms: u64,
    max_errors: u64,
}

struct ViewArgs {
    cache_key: Option<String>,
    root: Option<String>,
    path: Option<String>,
    depth: u32,
    list_limit: usize,
    slice_min_ratio: f64,
    max_slices: usize,
    max_flat: usize,
    metric: Metric,
    cross_fs: bool,
    count_hardlinks: bool,
}

struct StatArgs {
    path: Option<String>,
}

enum Command {
    Scan(ScanArgs),
    View(ViewArgs),
    Stat(StatArgs),
    Proto,
}

struct Args {
    cache_dir: Option<PathBuf>,
    command: Command,
}

fn usage() -> ! {
    let _ = writeln!(
        io::stderr(),
        "\
Usage: omadisk-scan <command> [options]

commands:
  scan    Walk a tree, stream NDJSON, atomically write the cache
  view    Print one view event from a cache
  stat    Print one JSON object: used/free/total for a path's filesystem
  proto   Print protocol version, mkdir the cache dir, and exit

scan options:
  --root PATH              default: $HOME
  --metric allocated|apparent    default: allocated
  --cross-fs               allow crossing mount points
  --follow-dir-symlinks    follow directory symlinks
  --count-hardlinks        count every hard link fully
  --ignore PATH            extra absolute prefix to skip (repeatable)
  --emit-view-ms N         view snapshot period, default 500; 0 = only at end
  --progress-ms N          progress period, default 250
  --cache-dir DIR
  --max-errors N           abort after N fatal I/O errors (default: 10000)

view options:
  --cache-key KEY
  --root PATH
  --path PATH
  --depth N
  --list-limit N
  --slice-min-ratio R
  --max-slices N
  --max-flat N
  --metric allocated|apparent
  --cross-fs
  --count-hardlinks
  --cache-dir DIR

stat options:
  --path PATH              default: $HOME
"
    );
    std::process::exit(EXIT_USAGE);
}

fn take_value(args: &[String], i: &mut usize) -> Option<String> {
    if *i + 1 >= args.len() {
        return None;
    }
    *i += 1;
    Some(args[*i].clone())
}

fn parse_eq(arg: &str, name: &str) -> Option<String> {
    arg.strip_prefix(name)?
        .strip_prefix('=')
        .map(str::to_string)
}

fn parse_args(argv: &[String]) -> Result<Args, i32> {
    let mut cache_dir = None;
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if a == "--help" || a == "-h" {
            let _ = writeln!(io::stderr(), "omadisk-scan scan|view|stat|proto");
            std::process::exit(0);
        }
        if a == "--cache-dir" {
            cache_dir = Some(PathBuf::from(take_value(argv, &mut i).ok_or(EXIT_USAGE)?));
            i += 1;
            continue;
        }
        if let Some(v) = parse_eq(a, "--cache-dir") {
            cache_dir = Some(PathBuf::from(v));
            i += 1;
            continue;
        }
        break;
    }
    if i >= argv.len() {
        return Err(EXIT_USAGE);
    }
    let cmd = argv[i].as_str();
    i += 1;
    match cmd {
        "scan" => {
            let mut a = ScanArgs {
                root: None,
                metric: Metric::Allocated,
                cross_fs: false,
                follow_dir_symlinks: false,
                count_hardlinks: false,
                ignore: Vec::new(),
                emit_view_ms: 500,
                progress_ms: 250,
                max_errors: 10_000,
            };
            while i < argv.len() {
                let arg = &argv[i];
                if arg == "--root" {
                    a.root = Some(take_value(argv, &mut i).ok_or(EXIT_USAGE)?);
                } else if let Some(v) = parse_eq(arg, "--root") {
                    a.root = Some(v);
                } else if arg == "--metric" {
                    a.metric = Metric::parse(&take_value(argv, &mut i).ok_or(EXIT_USAGE)?)
                        .ok_or(EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--metric") {
                    a.metric = Metric::parse(&v).ok_or(EXIT_USAGE)?;
                } else if arg == "--cross-fs" {
                    a.cross_fs = true;
                } else if arg == "--follow-dir-symlinks" {
                    a.follow_dir_symlinks = true;
                } else if arg == "--count-hardlinks" {
                    a.count_hardlinks = true;
                } else if arg == "--ignore" {
                    a.ignore.push(take_value(argv, &mut i).ok_or(EXIT_USAGE)?);
                } else if let Some(v) = parse_eq(arg, "--ignore") {
                    a.ignore.push(v);
                } else if arg == "--emit-view-ms" {
                    a.emit_view_ms = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--emit-view-ms") {
                    a.emit_view_ms = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--progress-ms" {
                    a.progress_ms = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--progress-ms") {
                    a.progress_ms = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--max-errors" {
                    a.max_errors = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--max-errors") {
                    a.max_errors = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--cache-dir" {
                    cache_dir = Some(PathBuf::from(take_value(argv, &mut i).ok_or(EXIT_USAGE)?));
                } else if let Some(v) = parse_eq(arg, "--cache-dir") {
                    cache_dir = Some(PathBuf::from(v));
                } else {
                    return Err(EXIT_USAGE);
                }
                i += 1;
            }
            Ok(Args {
                cache_dir,
                command: Command::Scan(a),
            })
        }
        "view" => {
            let mut a = ViewArgs {
                cache_key: None,
                root: None,
                path: None,
                depth: DEFAULT_DEPTH,
                list_limit: DEFAULT_LIST_LIMIT,
                slice_min_ratio: DEFAULT_SLICE_MIN_RATIO,
                max_slices: DEFAULT_MAX_SLICES,
                max_flat: DEFAULT_MAX_FLAT,
                metric: Metric::Allocated,
                cross_fs: false,
                count_hardlinks: false,
            };
            while i < argv.len() {
                let arg = &argv[i];
                if arg == "--cache-key" {
                    a.cache_key = Some(take_value(argv, &mut i).ok_or(EXIT_USAGE)?);
                } else if let Some(v) = parse_eq(arg, "--cache-key") {
                    a.cache_key = Some(v);
                } else if arg == "--root" {
                    a.root = Some(take_value(argv, &mut i).ok_or(EXIT_USAGE)?);
                } else if let Some(v) = parse_eq(arg, "--root") {
                    a.root = Some(v);
                } else if arg == "--path" {
                    a.path = Some(take_value(argv, &mut i).ok_or(EXIT_USAGE)?);
                } else if let Some(v) = parse_eq(arg, "--path") {
                    a.path = Some(v);
                } else if arg == "--depth" {
                    a.depth = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--depth") {
                    a.depth = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--list-limit" {
                    a.list_limit = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--list-limit") {
                    a.list_limit = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--slice-min-ratio" {
                    a.slice_min_ratio = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--slice-min-ratio") {
                    a.slice_min_ratio = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--max-slices" {
                    a.max_slices = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--max-slices") {
                    a.max_slices = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--max-flat" {
                    a.max_flat = take_value(argv, &mut i)
                        .ok_or(EXIT_USAGE)?
                        .parse()
                        .map_err(|_| EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--max-flat") {
                    a.max_flat = v.parse().map_err(|_| EXIT_USAGE)?;
                } else if arg == "--metric" {
                    a.metric = Metric::parse(&take_value(argv, &mut i).ok_or(EXIT_USAGE)?)
                        .ok_or(EXIT_USAGE)?;
                } else if let Some(v) = parse_eq(arg, "--metric") {
                    a.metric = Metric::parse(&v).ok_or(EXIT_USAGE)?;
                } else if arg == "--cross-fs" {
                    a.cross_fs = true;
                } else if arg == "--count-hardlinks" {
                    a.count_hardlinks = true;
                } else if arg == "--cache-dir" {
                    cache_dir = Some(PathBuf::from(take_value(argv, &mut i).ok_or(EXIT_USAGE)?));
                } else if let Some(v) = parse_eq(arg, "--cache-dir") {
                    cache_dir = Some(PathBuf::from(v));
                } else {
                    return Err(EXIT_USAGE);
                }
                i += 1;
            }
            Ok(Args {
                cache_dir,
                command: Command::View(a),
            })
        }
        "stat" => {
            let mut a = StatArgs { path: None };
            while i < argv.len() {
                let arg = &argv[i];
                if arg == "--path" {
                    a.path = Some(take_value(argv, &mut i).ok_or(EXIT_USAGE)?);
                } else if let Some(v) = parse_eq(arg, "--path") {
                    a.path = Some(v);
                } else if arg == "--cache-dir" {
                    cache_dir = Some(PathBuf::from(take_value(argv, &mut i).ok_or(EXIT_USAGE)?));
                } else if let Some(v) = parse_eq(arg, "--cache-dir") {
                    cache_dir = Some(PathBuf::from(v));
                } else {
                    return Err(EXIT_USAGE);
                }
                i += 1;
            }
            Ok(Args {
                cache_dir,
                command: Command::Stat(a),
            })
        }
        "proto" => {
            while i < argv.len() {
                let arg = &argv[i];
                if arg == "--cache-dir" {
                    cache_dir = Some(PathBuf::from(take_value(argv, &mut i).ok_or(EXIT_USAGE)?));
                } else if let Some(v) = parse_eq(arg, "--cache-dir") {
                    cache_dir = Some(PathBuf::from(v));
                } else {
                    return Err(EXIT_USAGE);
                }
                i += 1;
            }
            Ok(Args {
                cache_dir,
                command: Command::Proto,
            })
        }
        _ => Err(EXIT_USAGE),
    }
}

fn cmd_proto(cache_dir: &Path, process_start: SystemTime) -> i32 {
    let dir = match ensure_dir(cache_dir) {
        Ok(d) => d,
        Err(e) => {
            log_err(&format!("cannot create cache dir: {e}"));
            return EXIT_USAGE;
        }
    };
    sweep_tmps(&dir, process_start);
    cache::ensure_session(&dir);
    if emit(&proto_event(&dir.to_string_lossy())).is_err() {
        return EXIT_OK;
    }
    EXIT_OK
}

fn cmd_stat(cache_dir: &Path, process_start: SystemTime, args: StatArgs) -> i32 {
    let dir = match ensure_dir(cache_dir) {
        Ok(d) => d,
        Err(_) => return EXIT_USAGE,
    };
    sweep_tmps(&dir, process_start);
    let path = args.path.unwrap_or_else(home);
    if has_nul(&path) {
        return EXIT_USAGE;
    }
    if emit(&statfs::stat_path(&path)).is_err() {
        return EXIT_OK;
    }
    EXIT_OK
}

fn resolve_key(
    explicit: Option<&str>,
    root: &str,
    metric: Metric,
    stay: bool,
    hard: bool,
) -> String {
    if let Some(k) = explicit {
        return k.to_string();
    }
    let real = Path::new(root)
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| root.to_string());
    cache_key(&real, metric.as_str(), stay, hard)
}

fn cmd_view(cache_dir: &Path, process_start: SystemTime, args: ViewArgs) -> i32 {
    let dir = match ensure_dir(cache_dir) {
        Ok(d) => d,
        Err(_) => return EXIT_USAGE,
    };
    sweep_tmps(&dir, process_start);
    let root = args.root.clone().unwrap_or_else(home);
    let path = args.path.clone().unwrap_or_else(|| root.clone());
    if !is_abs(&root) || has_nul(&root) || has_nul(&path) {
        return EXIT_USAGE;
    }
    if !Path::new(&root).exists() {
        return EXIT_ROOT;
    }
    let stay = !args.cross_fs;
    let hard = !args.count_hardlinks;
    let key = resolve_key(args.cache_key.as_deref(), &root, args.metric, stay, hard);
    let Some((meta, tree)) = load_scan(&dir, &key) else {
        return EXIT_CACHE;
    };
    let meta_root = meta.get("root").and_then(|v| v.as_str()).unwrap_or(&root);
    let meta_real = meta
        .get("rootRealpath")
        .and_then(|v| v.as_str())
        .unwrap_or(meta_root);
    let mut path = remap_cached_path(&path, meta_root, meta_real);
    let adapter = CacheTree::new(tree);
    if adapter.get(&path).is_none() {
        let fallback = remap_cached_path(meta_root, meta_root, meta_real);
        if adapter.get(&fallback).is_none() {
            return EXIT_CACHE;
        }
        path = fallback;
    }
    let mut event = build_view(
        &adapter,
        &path,
        &ViewOpts {
            depth: args.depth,
            list_limit: args.list_limit,
            slice_min_ratio: args.slice_min_ratio,
            max_slices: args.max_slices,
            max_flat: args.max_flat,
            files: meta.get("files").and_then(|v| v.as_u64()).unwrap_or(0),
            dirs: meta.get("dirs").and_then(|v| v.as_u64()).unwrap_or(0),
            partial: false,
        },
    );
    if let Some(finished) = meta.get("finishedAt") {
        event["finishedAt"] = finished.clone();
    }
    if emit(&event).is_err() {
        return EXIT_OK;
    }
    EXIT_OK
}

fn cmd_scan(cache_dir: &Path, process_start: SystemTime, args: ScanArgs) -> i32 {
    let dir = match ensure_dir(cache_dir) {
        Ok(d) => d,
        Err(_) => return EXIT_USAGE,
    };
    sweep_tmps(&dir, process_start);
    let root = args.root.clone().unwrap_or_else(home);
    if root.is_empty() || has_nul(&root) || !is_abs(&root) {
        return EXIT_USAGE;
    }
    if !Path::new(&root).is_dir() {
        return EXIT_ROOT;
    }
    let metric = args.metric;
    let stay = !args.cross_fs;
    let hard_aware = !args.count_hardlinks;
    let real = Path::new(&root)
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| root.clone());
    let key = cache_key(&real, metric.as_str(), stay, hard_aware);
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let t0 = Instant::now();
    if emit(&hello(
        std::process::id(),
        &root,
        metric.as_str(),
        stay,
        hard_aware,
        started_at,
        &key,
    ))
    .is_err()
    {
        return EXIT_OK;
    }
    log_err(&format!("hello root={root} metric={}", metric.as_str()));

    let emit_view_s = args.emit_view_ms as f64 / 1000.0;
    let progress_s = args.progress_ms as f64 / 1000.0;
    let last_progress_at = Cell::new(0.0f64);
    let last_view_at = Cell::new(0.0f64);
    let last_counters = Cell::new((-1i64, -1i64, -1i64));
    let last_stderr_progress = Cell::new(0.0f64);
    let t_scan = Instant::now();

    let mut walker = Walker::new(&root);
    walker.metric = metric;
    walker.stay_on_fs = stay;
    walker.follow_dir_symlinks = args.follow_dir_symlinks;
    walker.count_hardlinks = args.count_hardlinks;
    walker.ignore = args
        .ignore
        .iter()
        .map(|p| {
            Path::new(p)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(p))
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    walker.max_errors = args.max_errors;
    walker.set_signal_check(signaled);

    walker.on_skip = Some(Box::new(|path, reason| {
        let _ = emit(&skip(path, reason));
        log_err(&format!("skip path={path} reason={reason}"));
    }));
    walker.on_error = Some(Box::new(|path, message, fatal| {
        let _ = emit(&error(path, message, fatal));
    }));

    walker.on_dirty = Some(Box::new(move |w| {
        if signaled() {
            return;
        }
        let now = t_scan.elapsed().as_secs_f64();
        let counters = (w.files as i64, w.dirs as i64, w.skipped as i64);
        if progress_s > 0.0
            && now - last_progress_at.get() >= progress_s
            && counters != last_counters.get()
        {
            let bytes_total = w.tree.nodes.get(w.tree.root).map(|n| n.bytes).unwrap_or(0);
            let _ = emit(&progress(
                w.files,
                w.dirs,
                bytes_total,
                &w.current,
                w.skipped,
            ));
            if now - last_stderr_progress.get() >= 2.0 {
                log_err(&format!(
                    "progress files={} dirs={} bytes={bytes_total}",
                    w.files, w.dirs
                ));
                last_stderr_progress.set(now);
            }
            last_progress_at.set(now);
            last_counters.set(counters);
        }
        if emit_view_s > 0.0 && now - last_view_at.get() >= emit_view_s {
            if !w.tree.nodes.is_empty() {
                let mut event = build_view(
                    &MemoryTree::new(w.tree.clone()),
                    &w.root.to_string_lossy(),
                    &ViewOpts {
                        files: w.files,
                        dirs: w.dirs,
                        partial: true,
                        ..ViewOpts::default()
                    },
                );
                event["partial"] = serde_json::json!(true);
                let _ = emit(&event);
            }
            last_view_at.set(now);
        }
    }));

    match walker.walk() {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::InvalidInput => return EXIT_ROOT,
        Err(e) => {
            let _ = emit(&error(&root, &format!("Error: {e}"), true));
            return EXIT_ROOT;
        }
    }

    if signaled() {
        return EXIT_SIGNAL;
    }
    if walker.aborted && signaled() {
        return EXIT_SIGNAL;
    }
    if walker.aborted {
        let _ = emit(&error(&root, "too many I/O errors", true));
        return EXIT_ABORT;
    }

    let elapsed_ms = t0.elapsed().as_millis() as u64;
    let finished_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let bytes_total = walker
        .tree
        .nodes
        .get(walker.tree.root)
        .map(|n| n.bytes)
        .unwrap_or(0);
    let meta = serde_json::json!({
        "v": 1,
        "key": key,
        "root": root,
        "rootRealpath": real,
        "metric": metric.as_str(),
        "stayOnFilesystem": stay,
        "hardlinkAware": hard_aware,
        "startedAt": started_at,
        "finishedAt": finished_at,
        "elapsedMs": elapsed_ms,
        "files": walker.files,
        "dirs": walker.dirs,
        "bytes": bytes_total,
        "skipped": walker.skipped,
        "errors": walker.errors,
        "protocol": 1,
    });
    let tree = serialize_tree(&walker.tree);
    if let Err(e) = publish_scan(&dir, &key, &meta, &tree) {
        let _ = emit(&error(&root, &format!("cache write: {e}"), true));
        return EXIT_ABORT;
    }

    if !walker.tree.nodes.is_empty() {
        let mut event = build_view(
            &MemoryTree::new(walker.tree.clone()),
            &walker.root.to_string_lossy(),
            &ViewOpts {
                files: walker.files,
                dirs: walker.dirs,
                partial: false,
                ..ViewOpts::default()
            },
        );
        event["partial"] = serde_json::json!(false);
        let _ = emit(&event);
    }
    let _ = emit(&done(
        walker.files,
        walker.dirs,
        bytes_total,
        elapsed_ms,
        walker.skipped,
        walker.errors,
        &key,
        false,
    ));
    log_err(&format!(
        "done elapsed_ms={elapsed_ms} files={} cache={}…",
        walker.files,
        &key[..key.len().min(4)]
    ));
    EXIT_OK
}

fn main() {
    install_signals();
    let process_start = SystemTime::now();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(code) => {
            if code == EXIT_USAGE {
                usage();
            }
            std::process::exit(code);
        }
    };
    let cache_dir = args.cache_dir.unwrap_or_else(default_cache_dir);
    if signaled() {
        std::process::exit(EXIT_SIGNAL);
    }
    let code = match args.command {
        Command::Scan(a) => cmd_scan(&cache_dir, process_start, a),
        Command::View(a) => cmd_view(&cache_dir, process_start, a),
        Command::Stat(a) => cmd_stat(&cache_dir, process_start, a),
        Command::Proto => cmd_proto(&cache_dir, process_start),
    };
    std::process::exit(code);
}
