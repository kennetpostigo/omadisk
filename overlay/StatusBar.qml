import QtQuick
import qs.Commons
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.menu.text
  implicitHeight: Style.space(28)

  readonly property string leftText: {
    if (!overlay) return ""
    if (overlay.error)
      return overlay.error
    if (overlay.scanning) {
      var p = overlay.progress || {}
      return "Scanning… " + Format.count(p.files || 0) + " files · " + Format.bytes(p.bytes || 0)
    }
    if (overlay.cacheAgeSec >= 0) {
      var age = "Scanned " + Format.relativeTime(overlay.cacheAgeSec)
      if (overlay.cacheAgeSec >= 86400)
        return age + " — press r to refresh"
      return age
    }
    return overlay.partial ? "Partial view" : ""
  }

  readonly property string rightText: {
    if (!overlay || !overlay.diskStat || !overlay.diskStat.ok) return ""
    var s = overlay.diskStat
    var label = overlay.isHomeRoot() ? "Home" : (overlay.scanRoot || "")
    return label + " · " + Format.bytes(overlay.currentView ? overlay.currentView.bytes : 0)
      + " analyzed · " + Format.bytes(s.free) + " free on disk"
  }

  Row {
    anchors.fill: parent
    spacing: Style.space(12)

    Text {
      width: parent.width * 0.45
      anchors.verticalCenter: parent.verticalCenter
      text: root.leftText
      color: overlay && overlay.error ? Color.urgent : root.foreground
      opacity: overlay && overlay.error ? 1 : 0.7
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
    }

    Text {
      width: parent.width * 0.38
      anchors.verticalCenter: parent.verticalCenter
      text: root.rightText
      color: root.foreground
      opacity: 0.7
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
      horizontalAlignment: Text.AlignRight
    }

    Text {
      anchors.verticalCenter: parent.verticalCenter
      text: "Rescan"
      color: Color.accent
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.caption
      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: if (overlay) overlay.startScan({ cancelLive: true })
      }
    }
  }
}
