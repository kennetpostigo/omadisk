.pragma library

function parseStat(line) {
  try {
    var obj = JSON.parse(String(line || ""))
    if (!obj || obj.v !== 1 || obj.type !== "stat") return null
    return obj
  } catch (e) {
    return null
  }
}

function bytes(n) {
  var v = Number(n)
  if (!isFinite(v) || v < 0) v = 0
  var units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"]
  var i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  if (i === 0) return Math.round(v) + " B"
  var digits = v >= 100 ? 0 : 1
  return v.toFixed(digits) + " " + units[i]
}

function chipIcon() {
  return "󰋊"
}

function availableText(stat) {
  if (!stat || !stat.ok) return ""
  return bytes(stat.free)
}

function chipText(stat, showAvailable, vertical) {
  return chipIcon()
}

// Bar/Button tooltips use QML AutoText. Neutralize tag/markdown image
// markers so a crafted configured path cannot fetch remote images.
function asPlainLabel(s) {
  return String(s || "")
    .replace(/</g, "\uFF1C")
    .replace(/>/g, "\uFF1E")
    .replace(/!\[/g, "!\u200B[")
}

function chipTooltip(stat, rootPath) {
  var path = asPlainLabel(rootPath)
  if (!stat || !stat.ok)
    return path + " · click to open Omadisk"
  return path + " · " + bytes(stat.free) + " available · click to open Omadisk"
}

function urgent(stat) {
  if (!stat || !stat.ok || !stat.total) return false
  return (Number(stat.used) / Number(stat.total)) > 0.9
}
