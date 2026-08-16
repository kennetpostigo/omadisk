import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

BarWidget {
  id: root
  moduleName: "postman.omadisk"

  property var stat: null

  function chipRoot() {
    var configured = setting("root", "")
    if (configured && String(configured).length > 0) return String(configured)
    return Quickshell.env("HOME") || "/home"
  }

  function scannerPath() {
    var url = String(Qt.resolvedUrl("../target/release/omadisk-scan"))
    return url.replace(/^file:\/\//, "")
  }

  function refresh() {
    statProc.command = [scannerPath(), "stat", "--path", chipRoot()]
    statProc.running = true
  }

  function summonOverlay() {
    var payload = JSON.stringify({ root: chipRoot() })
    if (root.bar && root.bar.shell && typeof root.bar.shell.summon === "function")
      root.bar.shell.summon("postman.omadisk", payload)
  }

  readonly property bool showFree: setting("showFree", true)
  readonly property int refreshSec: Math.max(10, Number(setting("refreshIntervalSec", 30)) || 30)
  readonly property string label: Model.chipText(stat, showFree, vertical)
  readonly property bool diskUrgent: Model.urgent(stat)

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()

  Timer {
    interval: root.refreshSec * 1000
    running: true
    repeat: true
    onTriggered: root.refresh()
  }

  Process {
    id: statProc
    stdout: SplitParser {
      onRead: function(line) {
        var ev = Model.parseStat(line)
        if (ev) root.stat = ev
      }
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.label
    tooltipText: root.chipRoot() + " · click to open Omadisk"
    active: root.diskUrgent
    foreground: root.diskUrgent ? Color.urgent : (bar ? bar.barForeground : Color.foreground)
    onPressed: function(b) {
      if (b === Qt.MiddleButton) root.refresh()
      else root.summonOverlay()
    }
  }
}
