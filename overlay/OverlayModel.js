.pragma library

var OTHER_SUFFIX = "/\0other"

function otherPath(parentPath) {
  return String(parentPath || "") + OTHER_SUFFIX
}

function isOtherPath(path) {
  var s = String(path || "")
  return s.length >= OTHER_SUFFIX.length
    && s.lastIndexOf(OTHER_SUFFIX) === s.length - OTHER_SUFFIX.length
}

function parseLine(line) {
  var text = String(line || "").replace(/^\s+|\s+$/g, "")
  if (!text) return null
  var obj
  try {
    obj = JSON.parse(text)
  } catch (e) {
    return null
  }
  if (!obj || typeof obj !== "object") return null
  if (obj.v !== undefined && obj.v !== 1) return { __badVersion: true, raw: obj }
  return obj
}

function countSliceNodes(view) {
  var count = 0
  function walk(node) {
    var children = (node && node.children) || []
    for (var i = 0; i < children.length; i++) {
      count++
      walk(children[i])
    }
  }
  if (view) walk(view)
  return count
}

function lerp(a, b, t) {
  return a + (b - a) * t
}

function hexRgb(hex) {
  var s = String(hex || "").replace("#", "")
  return {
    r: parseInt(s.substring(0, 2), 16),
    g: parseInt(s.substring(2, 4), 16),
    b: parseInt(s.substring(4, 6), 16)
  }
}

var PALETTE = [
  "#3B82F6",
  "#22C55E",
  "#F59E0B",
  "#EF4444",
  "#A855F7",
  "#06B6D4",
  "#F97316",
  "#EC4899",
  "#84CC16",
  "#14B8A6"
]
var OTHER_FILL = "#64748B"

function rgbToHex(r, g, b) {
  return "#" + [r, g, b].map(function(n) {
    var v = Math.max(0, Math.min(255, Math.round(n)))
    var h = v.toString(16)
    return h.length === 1 ? "0" + h : h
  }).join("")
}

function mixHex(hex, toward, t) {
  var a = hexRgb(hex)
  var b = hexRgb(toward)
  return rgbToHex(lerp(a.r, b.r, t), lerp(a.g, b.g, t), lerp(a.b, b.b, t))
}

function folderFill(kind, ring, paletteIndex) {
  if (kind === "other") return OTHER_FILL
  var base = PALETTE[((paletteIndex % PALETTE.length) + PALETTE.length) % PALETTE.length]
  if (ring <= 1) return base
  if (ring === 2) return mixHex(base, "#0b0f14", 0.18)
  return mixHex(base, "#0b0f14", 0.32)
}

function layoutSlices(view, startAtDeg, geom) {
  if (!view || !isFinite(view.bytes)) return []
  var out = []
  layoutNode(view, 1, startAtDeg, 360, geom, out, 0)
  return out
}

function layoutNode(node, ring, startDeg, parentSweep, geom, out, inheritedPalette) {
  if (ring > 3 || parentSweep <= 0) return
  var children = node.children || []
  var childTotal = 0
  for (var i = 0; i < children.length; i++) childTotal += Number(children[i].bytes) || 0
  if (childTotal <= 0) return

  var cursor = startDeg
  var minSweep = Number(geom.minSweepDeg)
  if (!isFinite(minSweep) || minSweep <= 0) minSweep = 2
  for (var j = 0; j < children.length; j++) {
    var c = children[j]
    var raw = parentSweep * ((Number(c.bytes) || 0) / childTotal)
    var gap = Math.min(Number(geom.sliceGapDeg) || 0.5, raw * 0.25)
    var usable = Math.max(0, raw - gap)
    if (c.kind === "other") usable = Math.max(usable, minSweep * 0.5)
    if (usable < minSweep && c.kind !== "other") {
      cursor += raw
      continue
    }
    var sliceStart = cursor + gap / 2
    var paletteIndex = ring === 1 ? j : inheritedPalette
    var innerR = geom.hubRadius + (ring - 1) * (geom.ringWidth + geom.ringPad)
    var outerR = innerR + geom.ringWidth
    var slicePath = c.kind === "other" ? (c.path || otherPath(node.path)) : (c.path || "")
    out.push({
      path: slicePath,
      name: c.name || "",
      kind: c.kind || "",
      bytes: Number(c.bytes) || 0,
      ring: ring,
      startDeg: sliceStart,
      sweepDeg: usable,
      innerR: innerR,
      outerR: outerR,
      fill: folderFill(c.kind, ring, paletteIndex),
      drillable: c.kind === "dir" && (Number(c.bytes) || 0) > 0 && !isOtherPath(slicePath)
    })
    if (c.kind === "dir" && usable >= minSweep)
      layoutNode(c, ring + 1, sliceStart, usable, geom, out, paletteIndex)
    cursor += raw
  }
}

function findNode(view, path) {
  if (!view || !path) return null
  if (view.path === path) return view
  var children = view.children || []
  for (var i = 0; i < children.length; i++) {
    if (children[i].path === path) return children[i]
    var nested = findNode(children[i], path)
    if (nested) return nested
  }
  return null
}

function flattenOneLevel(children) {
  var out = []
  for (var i = 0; i < (children || []).length; i++) {
    var c = children[i]
    if (c.kind === "other") continue
    out.push({
      name: c.name,
      path: c.path,
      kind: c.kind,
      bytes: c.bytes,
      partial: !!c.partial,
      error: c.error || "",
      childCount: c.childCount || 0
    })
  }
  return out
}

function inWindow(rootView, path) {
  if (!rootView || !path || isOtherPath(path)) return false
  if (path === rootView.path) return true
  return !!findNode(rootView, path)
}

function project(rootView, path) {
  var node = findNode(rootView, path)
  if (!node) return null
  return {
    v: 1,
    type: "view",
    path: node.path,
    name: node.name,
    bytes: node.bytes,
    apparent: node.apparent || node.bytes,
    partial: !!node.partial,
    files: 0,
    dirs: 0,
    listTruncated: 0,
    children: node.children || [],
    list: node.list || flattenOneLevel(node.children || [])
  }
}

function pathKeySet(slices) {
  var o = {}
  for (var i = 0; i < slices.length; i++) o[slices[i].ring + ":" + slices[i].path] = true
  return o
}

function pathKeySetFromModel(model) {
  var o = {}
  for (var i = 0; i < model.count; i++) {
    var s = model.get(i)
    o[s.ring + ":" + s.path] = true
  }
  return o
}

function findSliceIndex(model, ring, path) {
  for (var i = 0; i < model.count; i++) {
    var s = model.get(i)
    if (s.ring === ring && s.path === path) return i
  }
  return -1
}

function hasSlicePath(model, path) {
  if (!model) return false
  if (Object.prototype.toString.call(model) === "[object Array]") {
    for (var a = 0; a < model.length; a++) {
      if (model[a].path === path) return true
    }
    return false
  }
  for (var i = 0; i < model.count; i++) {
    if (model.get(i).path === path) return true
  }
  return false
}

function fillForPath(slices, path) {
  for (var i = 0; i < (slices || []).length; i++) {
    if (slices[i].path === path) return slices[i].fill
  }
  return ""
}

function sliceByPath(slices, path) {
  for (var i = 0; i < (slices || []).length; i++) {
    if (slices[i].path === path) return slices[i]
  }
  return null
}

function hitTestSlices(x, y, cx, cy, slices, hubR) {
  var dx = x - cx, dy = y - cy
  var r = Math.sqrt(dx * dx + dy * dy)
  var list = slices || []
  var innerHub = Number(hubR) || 0
  var outer = innerHub
  for (var i = 0; i < list.length; i++) {
    if (Number(list[i].outerR) > outer) outer = Number(list[i].outerR)
    if (Number(list[i].innerR) > 0 && (innerHub <= 0 || Number(list[i].innerR) < innerHub))
      innerHub = Number(list[i].innerR)
  }
  if (outer <= 1) return { kind: "miss" }
  if (r < innerHub) return { kind: "hub" }
  if (r > outer + 2) return { kind: "miss" }
  var deg = Math.atan2(dy, dx) * 180 / Math.PI
  if (deg < 0) deg += 360
  var slop = 2
  var best = null
  var bestDist = 1e9
  for (var j = 0; j < list.length; j++) {
    var s = list[j]
    var inner = Number(s.innerR) || 0
    var outR = Number(s.outerR) || 0
    if (r < inner - slop || r >= outR + slop) continue
    if (!angleContains(s.startDeg, s.sweepDeg, deg)) continue
    var mid = (inner + outR) / 2
    var dist = Math.abs(r - mid)
    if (dist < bestDist) {
      bestDist = dist
      best = s
    }
  }
  if (best) return { kind: "slice", slice: best }
  return { kind: "miss" }
}

function replaceSliceModel(model, slices) {
  model.clear()
  for (var i = 0; i < slices.length; i++) model.append(slices[i])
}

function patchSlices(model, slices) {
  for (var i = 0; i < slices.length; i++) {
    var s = slices[i]
    var idx = findSliceIndex(model, s.ring, s.path)
    if (idx < 0) continue
    model.setProperty(idx, "startDeg", s.startDeg)
    model.setProperty(idx, "sweepDeg", s.sweepDeg)
    model.setProperty(idx, "bytes", s.bytes)
    model.setProperty(idx, "color", s.color)
    model.setProperty(idx, "innerR", s.innerR)
    model.setProperty(idx, "outerR", s.outerR)
    model.setProperty(idx, "name", s.name)
    model.setProperty(idx, "drillable", s.drillable)
  }
}

function samePathSet(a, b) {
  var ka = Object.keys(a), kb = Object.keys(b)
  if (ka.length !== kb.length) return false
  for (var i = 0; i < ka.length; i++) if (!b[ka[i]]) return false
  return true
}

function angleContains(startDeg, sweepDeg, deg) {
  var start = ((Number(startDeg) % 360) + 360) % 360
  var sweep = Number(sweepDeg) || 0
  var end = start + sweep
  if (end <= 360) return deg >= start && deg < end
  return deg >= start || deg < (end - 360)
}

function hitTest(x, y, cx, cy, model, hubR, outerR) {
  var dx = x - cx, dy = y - cy
  var r = Math.sqrt(dx * dx + dy * dy)
  var pad = 6
  var effectiveHub = Number(hubR) || 0
  var effectiveOuter = Number(outerR) || 0
  if (model && model.count > 0) {
    var minI = 1e9
    var maxO = 0
    for (var i = 0; i < model.count; i++) {
      var bounds = model.get(i)
      if (Number(bounds.innerR) < minI) minI = Number(bounds.innerR)
      if (Number(bounds.outerR) > maxO) maxO = Number(bounds.outerR)
    }
    if (minI < 1e9 && minI > 0) effectiveHub = minI
    if (maxO > 0) effectiveOuter = maxO
  }
  if (effectiveOuter <= 1) return { kind: "miss" }
  if (r < effectiveHub) return { kind: "hub" }
  if (r > effectiveOuter + pad) return { kind: "miss" }
  var deg = Math.atan2(dy, dx) * 180 / Math.PI
  if (deg < 0) deg += 360
  var best = null
  var bestDist = 1e9
  for (var j = 0; j < model.count; j++) {
    var s = model.get(j)
    if (!angleContains(s.startDeg, s.sweepDeg, deg)) continue
    var mid = ((Number(s.innerR) || 0) + (Number(s.outerR) || 0)) / 2
    var dist = Math.abs(r - mid)
    if (dist < bestDist) {
      bestDist = dist
      best = s
    }
  }
  if (best) return { kind: "slice", slice: best }
  return { kind: "miss" }
}

function parentPath(path) {
  var s = String(path || "")
  if (!s || s === "/") return ""
  var trimmed = s.replace(/\/+$/, "")
  var i = trimmed.lastIndexOf("/")
  if (i <= 0) return "/"
  return trimmed.substring(0, i)
}

function normalizePath(path) {
  var s = String(path || "")
  if (!s) return ""
  s = s.replace(/\/+/g, "/")
  if (s.length > 1) s = s.replace(/\/+$/, "")
  return s
}

function isValidAbsPath(path) {
  var s = String(path || "")
  return s.length > 0 && s.charAt(0) === "/" && s.indexOf("\0") < 0
}

function isDescendant(root, path) {
  var r = normalizePath(root)
  var p = normalizePath(path)
  if (!r || !p) return false
  if (p === r) return true
  if (r === "/") return p.charAt(0) === "/"
  return p.indexOf(r + "/") === 0
}

function joinUnder(root, name) {
  var r = normalizePath(root)
  if (r === "/") return "/" + name
  return r + "/" + name
}

function isListRowPath(rows, path) {
  for (var i = 0; i < (rows || []).length; i++) {
    if (rows[i].path === path) return true
  }
  return false
}

function basename(path) {
  var s = String(path || "").replace(/\/+$/, "")
  if (!s) return "/"
  var i = s.lastIndexOf("/")
  return i < 0 ? s : s.substring(i + 1)
}
