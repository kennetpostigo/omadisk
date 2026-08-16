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

function chipText(stat, showFree, vertical) {
  if (!stat || !stat.ok) return "Disk"
  if (vertical) return bytes(stat.used)
  if (showFree) return bytes(stat.used) + " · " + bytes(stat.free)
  return bytes(stat.used)
}

function urgent(stat) {
  if (!stat || !stat.ok || !stat.total) return false
  return (Number(stat.used) / Number(stat.total)) > 0.9
}
