import QtQuick
import qs.Commons
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text
  implicitHeight: Style.space(18)

  readonly property string text: {
    if (!overlay) return ""
    if (overlay.error) return overlay.error
    if (overlay.scanning) {
      var p = overlay.progress || {}
      return "scanning · " + Format.bytes(p.bytes || 0)
    }
    var parts = []
    if (overlay.diskStat && overlay.diskStat.ok)
      parts.push(Format.bytes(overlay.diskStat.free) + " free")
    if (overlay.cacheAgeSec >= 0)
      parts.push(Format.relativeTime(overlay.cacheAgeSec))
    return parts.join("  ·  ")
  }

  Text {
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    width: parent.width - rescan.implicitWidth - Style.space(12)
    text: root.text
    color: overlay && overlay.error ? Color.urgent : root.foreground
    opacity: overlay && overlay.error ? 1 : 0.4
    font.family: Style.font.family
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  Text {
    id: rescan
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    text: "↻"
    color: root.foreground
    opacity: rescanMouse.containsMouse ? 0.9 : 0.35
    font.family: Style.font.family
    font.pixelSize: Style.font.body
    MouseArea {
      id: rescanMouse
      anchors.fill: parent
      anchors.margins: -Style.space(4)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: if (overlay) overlay.startScan({ cancelLive: true })
    }
  }
}
