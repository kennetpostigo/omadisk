import Quickshell
import Quickshell.Io
import QtQuick
import qs.Commons
import qs.Ui
import "OverlayModel.js" as Model
import "Format.js" as Format

Item {
  id: root

  property var panelOwner: null
  property var bar: null
  property var manifest: null
  property bool opened: false

  property string scanRoot: ""
  property string focusPath: ""
  property var lastRootView: null
  property var currentView: null
  property var listRows: []
  property string hoverPath: ""
  property int hoverTick: 0
  property int selectedIndex: 0
  property bool scanning: false
  property bool cachePublished: false
  property bool deeperPending: false
  property bool expectedStop: false
  property bool pendingRun: false
  property bool expectedViewStop: false
  property var pendingView: null
  property string runningScanRoot: ""
  property bool offerHome: false
  property var progress: ({ files: 0, dirs: 0, bytes: 0, current: "" })
  property string error: ""
  property int cacheAgeSec: -1
  property bool partial: true
  property bool focusChanging: false
  property bool protoReady: false
  property bool ioniceAvailable: true
  property bool parseWarned: false
  property string scanStderr: ""
  property string viewStderr: ""
  property var diskStat: null
  property var sessionObj: null
  property real sunburstFade: 1
  property real cacheFinishedAt: 0

  readonly property color foreground: bar ? bar.foreground : Color.popups.text
  readonly property color selectedBackground: Util.alpha(Color.accent, 0.28)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  property var slices: []
  readonly property bool hoverHasSlice: Model.hasSlicePath(slices, hoverPath)
  readonly property bool hoverIsListRow: Model.isListRowPath(listRows, hoverPath)
  readonly property var hoverSlice: Model.sliceByPath(slices, hoverPath)

  function homeDir() {
    return Quickshell.env("HOME") || "/home"
  }

  function cacheDir() {
    var xdg = Quickshell.env("XDG_CACHE_HOME")
    if (xdg) return xdg + "/omadisk"
    return homeDir() + "/.cache/omadisk"
  }

  function scannerPath() {
    var dir = (root.manifest && root.manifest.__sourceDir) || ""
    if (dir)
      return String(dir).replace(/\/$/, "") + "/target/release/omadisk-scan"
    var url = String(Qt.resolvedUrl("../target/release/omadisk-scan"))
    return url.replace(/^file:\/\//, "")
  }

  function configuredRoot() {
    if (root.panelOwner && typeof root.panelOwner.setting === "function") {
      var configured = root.panelOwner.setting("root", "")
      if (configured && String(configured).length > 0) return String(configured)
    }
    return ""
  }

  function isHomeRoot() {
    return root.scanRoot === homeDir()
  }

  property real lastLayoutSize: 0

  function geom() {
    var size = Math.min(sunburst.width, sunburst.height)
    if (!(size > 8)) size = Style.space(240)
    return {
      sliceGapDeg: 0.6,
      minSweepDeg: 2.0,
      hubRadius: 0.36 * size / 2,
      ringWidth: 0.19 * size / 2,
      ringPad: Math.max(2, size * 0.012)
    }
  }

  function relayoutSlices() {
    var size = Math.min(sunburst.width, sunburst.height)
    if (!(size > 8) || !root.currentView) return
    if (Math.abs(size - root.lastLayoutSize) < 0.5) return
    root.lastLayoutSize = size
    root.slices = Model.layoutSlices(root.currentView, -90, geom())
  }

  function niceIoniceConcat(args) {
    var scanner = scannerPath()
    if (root.ioniceAvailable)
      return ["/usr/bin/nice", "-n", "10", "/usr/bin/ionice", "-c", "2", "-n", "7", scanner].concat(args)
    return ["/usr/bin/nice", "-n", "10", scanner].concat(args)
  }

  function parsePayload(payloadJson) {
    if (!payloadJson) return {}
    try {
      var obj = JSON.parse(payloadJson)
      return obj && typeof obj === "object" ? obj : {}
    } catch (e) {
      return {}
    }
  }

  function rejectRoot(path) {
    root.scanRoot = homeDir()
    root.focusPath = root.scanRoot
    root.error = "Path missing: " + path
    root.offerHome = true
    root.currentView = null
    root.listRows = []
    root.slices = []
  }

  function resolveRootAndFocus(payload) {
    var p = payload || {}
    if (typeof p.root === "string" && p.root.length > 0) {
      if (!Model.isValidAbsPath(p.root)) {
        rejectRoot(p.root)
        return
      }
      root.scanRoot = Model.normalizePath(p.root)
      var focus = p.focus || root.scanRoot
      if (!Model.isValidAbsPath(focus) || !Model.isDescendant(root.scanRoot, focus))
        focus = root.scanRoot
      else
        focus = Model.normalizePath(focus)
      root.focusPath = focus
      return
    }
    if (root.focusPath && root.scanRoot
        && Model.isValidAbsPath(root.focusPath)
        && Model.isDescendant(root.scanRoot, root.focusPath)) {
      return
    }
    if (root.sessionObj && root.sessionObj.v === 1 && root.sessionObj.lastRoot) {
      if (!Model.isValidAbsPath(root.sessionObj.lastRoot)) {
        rejectRoot(root.sessionObj.lastRoot)
        return
      }
      root.scanRoot = Model.normalizePath(root.sessionObj.lastRoot)
      var lastFocus = root.sessionObj.lastFocus
      if (lastFocus && Model.isValidAbsPath(lastFocus) && Model.isDescendant(root.scanRoot, lastFocus))
        root.focusPath = Model.normalizePath(lastFocus)
      else
        root.focusPath = root.scanRoot
      return
    }
    root.scanRoot = homeDir()
    root.focusPath = root.scanRoot
  }

  function persistSession() {
    if (!root.scanRoot || !root.focusPath) return
    var obj = {
      v: 1,
      lastRoot: root.scanRoot,
      lastFocus: root.focusPath,
      lastKey: "",
      lastOpenedAt: Math.floor(Date.now() / 1000)
    }
    root.sessionObj = obj
    if (!root.protoReady) return
    sessionFile.setText(JSON.stringify(obj) + "\n")
    Quickshell.execDetached(["chmod", "600", cacheDir() + "/session.json"])
  }

  function applyLine(line) {
    var ev = Model.parseLine(line)
    if (!ev) return
    if (ev.__badVersion) {
      root.error = "Unsupported scanner protocol"
      return
    }
    if (ev.type === "view") applyView(ev)
    else if (ev.type === "progress") {
      if (root.runningScanRoot && root.runningScanRoot !== root.scanRoot) return
      root.progress = {
        files: ev.files || 0,
        dirs: ev.dirs || 0,
        bytes: ev.bytes || 0,
        current: ev.current || ""
      }
    } else if (ev.type === "done") {
      if (root.runningScanRoot && root.runningScanRoot !== root.scanRoot) return
      root.cachePublished = true
      root.partial = ev.partial === true
    } else if (ev.type === "error" && ev.fatal) {
      root.error = ev.message || "Scan error"
    }
  }

  function setCurrentView(ev, changing) {
    var nextList = ev.list || []
    root.currentView = ev
    root.partial = ev.partial === true
    root.listRows = nextList
    root.slices = Model.layoutSlices(ev, -90, geom())
    if (changing) playFocusFade()
    if (root.selectedIndex >= nextList.length)
      root.selectedIndex = Math.max(0, nextList.length - 1)
    root.lastLayoutSize = Math.min(sunburst.width, sunburst.height)
    syncSelection(root.hoverPath || (nextList.length ? nextList[root.selectedIndex].path : ""))
  }

  function pathKind(path) {
    var i
    for (i = 0; i < root.listRows.length; i++) {
      if (root.listRows[i].path === path) return root.listRows[i].kind || ""
    }
    if (root.currentView) {
      var node = Model.findNode(root.currentView, path)
      if (node && node.kind) return node.kind
    }
    for (i = 0; i < root.slices.length; i++) {
      if (root.slices[i].path === path) return root.slices[i].kind || ""
    }
    return ""
  }

  function syncSelection(path) {
    root.hoverPath = path || ""
    root.hoverTick += 1
    if (!path) return
    for (var i = 0; i < root.listRows.length; i++) {
      if (root.listRows[i].path === path) {
        root.selectedIndex = i
        if (childList) childList.positionSelected()
        return
      }
    }
  }

  function isDrillable(path) {
    if (!path || Model.isOtherPath(path)) return false
    var kind = pathKind(path)
    if (kind === "dir" || kind === "mount") return true
    if (kind !== "") return false
    if (path === root.scanRoot) return true
    return !!(root.focusPath && Model.isDescendant(path, root.focusPath) && path !== root.focusPath)
  }

  function activatePath(path) {
    if (!path) return
    if (Model.isOtherPath(path)) {
      if (path === Model.otherPath(root.focusPath))
        syncSelection(path)
      return
    }
    syncSelection(path)
    if (isDrillable(path)) drill(path)
  }

  function playFocusFade() {
    root.sunburstFade = 0.88
    fadeTimer.restart()
  }

  function applyView(ev) {
    if (!ev || ev.v !== 1 || ev.type !== "view") return
    if (Model.countSliceNodes(ev) > 120) {
      if (!root.parseWarned) {
        console.warn("omadisk: view exceeded 120 slices, ignored")
        root.parseWarned = true
      }
      return
    }
    if ((ev.path === root.scanRoot || ev.path === root.focusPath) && ev.finishedAt)
      stampCacheAge(ev.finishedAt)
    if (ev.path === root.scanRoot)
      root.lastRootView = ev
    if (ev.path === root.focusPath) {
      root.error = ""
      root.offerHome = false
      setCurrentView(ev, false)
      return
    }
    if (ev.path === root.scanRoot && Model.inWindow(ev, root.focusPath)) {
      var projected = Model.project(ev, root.focusPath)
      if (projected) {
        root.error = ""
        root.offerHome = false
        setCurrentView(projected, false)
      }
      return
    }
    if (ev.path === root.scanRoot && root.focusPath !== root.scanRoot
        && !root.scanning && !root.deeperPending) {
      startViewProc(root.scanRoot, root.focusPath)
    }
  }

  function drill(path) {
    if (Model.isOtherPath(path)) {
      if (path === Model.otherPath(root.focusPath))
        syncSelection(path)
      return
    }
    if (!path) return
    if (!isDrillable(path)) {
      syncSelection(path)
      return
    }
    root.focusChanging = true
    root.focusPath = path
    if (Model.inWindow(root.lastRootView, path) || Model.inWindow(root.currentView, path)) {
      var src = Model.inWindow(root.lastRootView, path) ? root.lastRootView : root.currentView
      setCurrentView(Model.project(src, path), true)
      if (root.cachePublished)
        startViewProc(root.scanRoot, path)
      persistSession()
      root.focusChanging = false
      return
    }
    if (root.cachePublished) {
      startViewProc(root.scanRoot, path)
      persistSession()
      root.focusChanging = false
      return
    }
    root.deeperPending = true
    var row = null
    for (var i = 0; i < root.listRows.length; i++) {
      if (root.listRows[i].path === path) { row = root.listRows[i]; break }
    }
    if (!row && root.currentView)
      row = Model.findNode(root.currentView, path)
    setCurrentView({
      v: 1,
      type: "view",
      path: path,
      name: row ? row.name : Model.basename(path),
      bytes: row ? row.bytes : 0,
      apparent: row ? (row.apparent || row.bytes) : 0,
      partial: true,
      files: 0,
      dirs: 0,
      listTruncated: 0,
      children: [],
      list: []
    }, true)
    persistSession()
    root.focusChanging = false
  }

  function goUp() {
    if (!root.focusPath || root.focusPath === root.scanRoot) return
    var parent = Model.parentPath(root.focusPath)
    if (!parent || !Model.isDescendant(root.scanRoot, parent))
      parent = root.scanRoot
    drill(parent)
  }

  function onDone() {
    root.cachePublished = true
    root.scanning = false
    if (root.deeperPending || root.focusPath !== root.scanRoot) {
      startViewProc(root.scanRoot, root.focusPath)
      root.deeperPending = false
    }
    refreshStat()
    stampCacheAge(Date.now() / 1000)
  }

  function stampCacheAge(finishedAt) {
    var t = Number(finishedAt)
    if (!isFinite(t) || t <= 0) return
    root.cacheFinishedAt = t
    root.cacheAgeSec = Math.max(0, Math.floor(Date.now() / 1000 - t))
  }

  function startScan(opts) {
    opts = opts || {}
    root.error = ""
    root.offerHome = false
    root.scanning = true
    if (scanProc.running) {
      if (!opts.cancelLive && root.runningScanRoot === root.scanRoot)
        return
      root.pendingRun = true
      root.expectedStop = true
      scanProc.running = false
      return
    }
    root.expectedStop = false
    root.runningScanRoot = root.scanRoot
    scanProc.command = niceIoniceConcat(["scan", "--root", root.scanRoot, "--emit-view-ms", "500"])
    scanProc.running = true
    console.log("omadisk: scan start", root.scanRoot)
  }

  function attachOrStartScan() {
    if (scanProc.running && root.runningScanRoot === root.scanRoot) {
      root.scanning = true
      return
    }
    startScan({
      cancelLive: scanProc.running && root.runningScanRoot !== root.scanRoot
    })
  }

  function startViewProc(rootPath, path) {
    if (viewProc.running) {
      root.expectedViewStop = true
      root.pendingView = { rootPath: rootPath, path: path }
      viewProc.running = false
      return
    }
    root.pendingView = null
    viewProc.command = [scannerPath(), "view", "--root", rootPath, "--path", path, "--depth", "3"]
    viewProc.running = true
  }

  function onViewExited(exitCode) {
    if (root.pendingView) {
      var next = root.pendingView
      root.pendingView = null
      root.expectedViewStop = false
      Qt.callLater(function() { startViewProc(next.rootPath, next.path) })
      return
    }
    if (root.expectedViewStop) {
      root.expectedViewStop = false
      return
    }
    if (exitCode === 0) {
      root.cachePublished = true
      return
    }
    if (exitCode === 3) {
      if (root.focusPath !== root.scanRoot) {
        root.focusPath = root.scanRoot
        persistSession()
        startViewProc(root.scanRoot, root.scanRoot)
        return
      }
      root.cachePublished = false
      attachOrStartScan()
      return
    }
    if (exitCode === 2) {
      root.error = "Path missing: " + root.scanRoot
      root.offerHome = true
      return
    }
    if (exitCode === 127 && root.ioniceAvailable && String(viewProc.command[0] || "").indexOf("ionice") >= 0) {
      root.ioniceAvailable = false
      startViewProc(root.scanRoot, root.focusPath)
      return
    }
    var scanner = scannerPath()
    root.error = root.viewStderr || ("view failed — scanner missing at " + scanner + " — run mise install && ./scripts/build.sh")
  }

  function refreshStat() {
    if (!root.scanRoot) return
    statProc.command = [scannerPath(), "stat", "--path", root.scanRoot]
    statProc.running = true
  }

  function fallBackHome() {
    root.offerHome = false
    root.error = ""
    root.scanRoot = homeDir()
    root.focusPath = root.scanRoot
    persistSession()
    startViewProc(root.scanRoot, root.focusPath)
  }

  function pageRows() {
    return childList ? childList.pageRows() : 8
  }

  function moveSelection(delta) {
    var n = root.listRows.length
    if (n === 0) return
    var next = root.selectedIndex + delta
    if (next < 0) next = 0
    if (next >= n) next = n - 1
    syncSelection(root.listRows[next].path)
  }

  function activateSelected() {
    var row = root.listRows[root.selectedIndex]
    if (row) activatePath(row.path)
  }

  function openFocus() {
    if (root.focusPath)
      Quickshell.execDetached(["xdg-open", root.focusPath])
  }

  function copyFocus() {
    if (root.focusPath)
      Quickshell.execDetached(["wl-copy", root.focusPath])
  }

  function startSession(payloadJson) {
    root.error = ""
    root.offerHome = false
    var prevRoot = root.scanRoot
    var payload = parsePayload(payloadJson)
    if (!payload.root && root.configuredRoot())
      payload.root = root.configuredRoot()
    resolveRootAndFocus(payload)
    if (prevRoot && root.scanRoot !== prevRoot) {
      root.lastRootView = null
      root.deeperPending = false
      root.progress = ({ files: 0, dirs: 0, bytes: 0, current: "" })
      root.currentView = null
      root.listRows = []
      root.slices = []
      root.cachePublished = false
      root.cacheAgeSec = -1
      root.cacheFinishedAt = 0
    }
    root.scanning = scanProc.running && root.runningScanRoot === root.scanRoot
    console.log("omadisk: open", root.scanRoot, "rescan=" + (payload.rescan === true))
    if (root.offerHome)
      return
    startViewProc(root.scanRoot, root.focusPath)
    if (payload.rescan === true)
      startScan({ cancelLive: true })
    refreshStat()
  }

  function dismiss() {
    if (root.panelOwner && typeof root.panelOwner.close === "function")
      root.panelOwner.close()
  }

  function handleKey(event) {
    var t = event.text
    if (event.key === Qt.Key_Escape) {
      if (root.focusPath !== root.scanRoot) goUp()
      else dismiss()
      event.accepted = true
    } else if (event.key === Qt.Key_Down || t === "j") {
      moveSelection(1)
      event.accepted = true
    } else if (event.key === Qt.Key_Up || t === "k") {
      moveSelection(-1)
      event.accepted = true
    } else if (event.key === Qt.Key_Right || event.key === Qt.Key_Return || event.key === Qt.Key_Enter || t === "l") {
      activateSelected()
      event.accepted = true
    } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Backspace || t === "h") {
      goUp()
      event.accepted = true
    } else if (event.key === Qt.Key_Home) {
      if (root.listRows.length) {
        root.selectedIndex = 0
        root.hoverPath = root.listRows[0].path
        childList.positionSelected()
      }
      event.accepted = true
    } else if (event.key === Qt.Key_End) {
      if (root.listRows.length) {
        root.selectedIndex = root.listRows.length - 1
        root.hoverPath = root.listRows[root.selectedIndex].path
        childList.positionSelected()
      }
      event.accepted = true
    } else if (event.key === Qt.Key_PageUp) {
      moveSelection(-childList.pageRows())
      event.accepted = true
    } else if (event.key === Qt.Key_PageDown) {
      moveSelection(childList.pageRows())
      event.accepted = true
    } else if (t === "r") {
      startScan({ cancelLive: true })
      event.accepted = true
    } else if (t === "o") {
      openFocus()
      event.accepted = true
    } else if (t === "y") {
      copyFocus()
      event.accepted = true
    }
  }

  Component.onCompleted: {
    protoProc.command = [scannerPath(), "proto"]
    protoProc.running = true
  }

  Timer {
    id: fadeTimer
    interval: 20
    onTriggered: root.sunburstFade = 1
  }

  Timer {
    interval: 1000
    running: root.opened && root.cacheFinishedAt > 0
    repeat: true
    onTriggered: root.cacheAgeSec = Math.max(0, Math.floor(Date.now() / 1000 - root.cacheFinishedAt))
  }

  Process {
    id: protoProc
    stdout: SplitParser {
      onRead: function(line) {
        var ev = Model.parseLine(line)
        if (ev && ev.type === "proto") root.protoReady = true
      }
    }
    onExited: function(code) {
      if (code === 0) root.protoReady = true
    }
  }

  Process {
    id: scanProc
    stdout: SplitParser { onRead: function(line) { root.applyLine(line) } }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.scanStderr = String(text || "").trim()
    }
    onExited: function(exitCode) {
      var completedRoot = root.runningScanRoot
      if (exitCode === 127 && root.ioniceAvailable) {
        root.ioniceAvailable = false
        if (root.pendingRun || root.scanning) {
          root.pendingRun = false
          root.expectedStop = false
          Qt.callLater(function() { startScan({ cancelLive: false }) })
          return
        }
      }
      if (root.pendingRun) {
        root.pendingRun = false
        root.expectedStop = false
        Qt.callLater(function() { startScan({ cancelLive: false }) })
        return
      }
      if (!root.expectedStop && exitCode !== 0 && exitCode !== 130
          && completedRoot === root.scanRoot) {
        root.error = root.scanStderr || "Scan failed"
      }
      root.expectedStop = false
      root.runningScanRoot = ""
      root.scanning = false
      console.log("omadisk: scan exited", exitCode)
      if (exitCode === 0 && completedRoot === root.scanRoot) root.onDone()
    }
  }

  Process {
    id: viewProc
    stdout: SplitParser { onRead: function(line) { root.applyLine(line) } }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.viewStderr = String(text || "").trim()
    }
    onExited: function(exitCode) { root.onViewExited(exitCode) }
  }

  Process {
    id: statProc
    stdout: SplitParser {
      onRead: function(line) {
        var ev = Model.parseLine(line)
        if (ev && ev.type === "stat") root.diskStat = ev
      }
    }
  }

  FileView {
    id: sessionFile
    path: root.protoReady ? root.cacheDir() + "/session.json" : ""
    printErrors: false
    atomicWrites: true
    onLoaded: {
      try {
        var disk = JSON.parse(text() || "{}")
        if (root.sessionObj && root.sessionObj.lastOpenedAt
            && disk.lastOpenedAt && disk.lastOpenedAt < root.sessionObj.lastOpenedAt)
          return
        root.sessionObj = disk
      } catch (e) {
        if (!root.sessionObj) root.sessionObj = null
      }
    }
  }

  Column {
    id: column
    anchors.fill: parent
    spacing: Style.space(8)

    BreadcrumbBar {
      width: parent.width
      height: Style.space(32)
      overlay: root
      foreground: root.foreground
      z: 2
    }

    Row {
      width: parent.width
      height: parent.height - Style.space(32) - Style.space(18) - Style.space(8) * 2
      spacing: Style.space(16)

      SunburstCanvas {
        id: sunburst
        width: Math.min(parent.height, parent.width * 0.5)
        height: parent.height
        overlay: root
        foreground: root.foreground
        onWidthChanged: root.relayoutSlices()
        onHeightChanged: root.relayoutSlices()
      }

      ChildList {
        id: childList
        width: parent.width - sunburst.width - parent.spacing
        height: parent.height
        overlay: root
        foreground: root.foreground
        selectedBackground: root.selectedBackground
      }
    }

    StatusBar {
      width: parent.width
      overlay: root
      foreground: root.foreground
    }
  }
}
