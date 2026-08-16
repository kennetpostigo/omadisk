.pragma library

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

function percent(used, total) {
  var t = Number(total)
  var u = Number(used)
  if (!isFinite(t) || t <= 0 || !isFinite(u)) return ""
  return Math.min(100, Math.max(0, Math.round((u / t) * 100))) + "%"
}

function relativeTime(secAgo) {
  var s = Number(secAgo)
  if (!isFinite(s) || s < 0) return ""
  if (s < 60) return Math.round(s) + "s ago"
  if (s < 3600) return Math.round(s / 60) + "m ago"
  if (s < 86400) return Math.round(s / 3600) + "h ago"
  var days = Math.round(s / 86400)
  return days === 1 ? "1 day ago" : days + " days ago"
}

function count(n) {
  var v = Number(n) || 0
  return String(Math.round(v)).replace(/\B(?=(\d{3})+(?!\d))/g, ",")
}
