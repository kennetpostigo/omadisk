# Omadisk NDJSON protocol

Every stdout line is one JSON object. UTF-8, no pretty-print. `v` is the
protocol version. v1 clients reject `v !== 1`. Unknown `type` values are
ignored (forward compatible). stderr is human logs only.

Cache lives under `$XDG_CACHE_HOME/omadisk` or `~/.cache/omadisk`
(`0700` dirs, `0600` files). The shell never opens `tree.json`.

## Commands

```
omadisk-scan scan  [--root PATH] [--metric allocated|apparent]
omadisk-scan view  [--root PATH] [--path PATH] [--cache-key KEY]
omadisk-scan stat  [--path PATH]
omadisk-scan proto
```

Exit codes: `0` ok, `1` usage, `2` root missing, `3` cache miss/corrupt,
`4` aborted, `130` signal (partial cache is not published).

## Events

### `hello` — first line of `scan`

```json
{"v":1,"type":"hello","pid":4242,"root":"/home/postman","metric":"allocated","stayOnFilesystem":true,"hardlinkAware":true,"startedAt":1755302400,"cacheKey":"a1b2c3d4e5f67890"}
```

### `progress`

```json
{"v":1,"type":"progress","files":12040,"dirs":880,"bytes":448790528,"current":"/home/postman/.cache/mesa_shader_cache","skipped":2}
```

### `skip`

`reason` ∈ `permission` | `cycle` | `ignored` | `other-fs` | `not-a-directory` | `io`.

```json
{"v":1,"type":"skip","path":"/home/postman/.gvfs","reason":"permission"}
```

### `error`

```json
{"v":1,"type":"error","path":"/home/postman","message":"OSError: [Errno 5] Input/output error","fatal":false}
```

### `view`

`children` is collapsed for the sunburst (≤40 + Other per ring, ≤120 slice
nodes). `list` is the uncollapsed children of the focus path (≤200).
Other wedges use the synthetic path `parent + "/\u0000other"` and are
omitted from `list`.

```json
{"v":1,"type":"view","path":"/home/postman","name":"postman","bytes":448790528,"apparent":430000000,"partial":true,"files":12040,"dirs":880,"listTruncated":0,"children":[{"name":".config","path":"/home/postman/.config","kind":"dir","bytes":524288,"partial":true,"error":"","childCount":8,"children":[]}],"list":[{"name":".config","path":"/home/postman/.config","kind":"dir","bytes":524288,"partial":true,"error":"","childCount":8}]}
```

### `done`

```json
{"v":1,"type":"done","files":100715,"dirs":8421,"bytes":137438953472,"elapsedMs":12040,"skipped":14,"errors":0,"cacheKey":"a1b2c3d4e5f67890","partial":false}
```

### `stat`

```json
{"v":1,"type":"stat","path":"/home/postman","fsPath":"/","used":137438953472,"free":1876899999999,"total":2014338953471,"ok":true}
```

`used = (f_blocks - f_bfree) * f_frsize`, `free = f_bavail * f_frsize`,
`total = f_blocks * f_frsize`.

### `proto`

```json
{"v":1,"type":"proto","protocol":1,"cacheDir":"/home/postman/.cache/omadisk"}
```

## Cache

`key = sha256(f"{realpath(root)}\n{metric}\n{stay_on_fs}\n{hardlink_aware}")[:16]`

```
~/.cache/omadisk/
  session.json
  scans/<key>/
    meta.json
    tree.json
```

Atomic publish: write `*.tmp`, `fsync`, `os.replace`. More than 4 scan
slots: evict missing-meta first, then oldest `meta.finishedAt`.
