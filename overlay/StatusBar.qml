import QtQuick
import qs.Commons
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text
  implicitHeight: Style.space(14)

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
    anchors.fill: parent
    verticalAlignment: Text.AlignVCenter
    text: root.text
    textFormat: Text.PlainText
    color: overlay && overlay.error ? Color.urgent : root.foreground
    opacity: overlay && overlay.error ? 1 : 0.4
    font.family: Style.font.family
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }
}
