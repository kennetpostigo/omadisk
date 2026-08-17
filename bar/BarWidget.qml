import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model
import "../overlay" as Disk

Panel {
  id: root
  moduleName: "postman.omadisk"
  ipcTarget: "postman.omadisk"

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

  readonly property int refreshSec: Math.max(10, Number(setting("refreshIntervalSec", 30)) || 30)
  readonly property string label: Model.chipIcon()
  readonly property bool diskUrgent: Model.urgent(stat)
  readonly property bool vertical: bar ? bar.vertical : false

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()

  onOpenedChanged: {
    if (opened)
      explorer.startSession("{}")
    else
      explorer.persistSession()
  }

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

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.label
    tooltipText: Model.chipTooltip(stat, root.chipRoot())
    active: root.diskUrgent
    onPressed: function(b) {
      if (b === Qt.MiddleButton) root.refresh()
      else if (root.opened) root.close()
      else root.open()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    padding: Style.space(10)
    contentWidth: panel.fittedContentWidth(Style.space(500))
    contentHeight: panel.cappedContentHeight(Style.space(292))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) {
        if (dy !== 0) explorer.moveSelection(dy)
        else if (dx > 0) explorer.activateSelected()
        else if (dx < 0) explorer.goUp()
      }
      onActivateRequested: explorer.activateSelected()
      onCloseRequested: {
        if (explorer.focusPath !== explorer.scanRoot) explorer.goUp()
        else root.close()
      }
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) {
        if (t === "r" || t === "R") explorer.startScan({ cancelLive: true })
        else if (t === "o" || t === "O") explorer.openFocus()
        else if (t === "y" || t === "Y") explorer.copyFocus()
      }

      Disk.Overlay {
        id: explorer
        anchors.fill: parent
        opened: root.opened
        panelOwner: root
        bar: root.bar
      }
    }
  }
}
