import QtQuick
import qs.Commons
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text

  readonly property var view: overlay ? overlay.currentView : null

  Column {
    anchors.centerIn: parent
    width: parent.width * 0.82
    spacing: Style.space(2)

    Text {
      width: parent.width
      text: root.view ? Format.bytes(root.view.bytes) : (overlay && overlay.scanning ? "…" : "")
      color: root.foreground
      font.family: Style.font.family
      font.pixelSize: Style.font.title
      font.bold: true
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideRight
    }

    Text {
      width: parent.width
      text: {
        if (!overlay) return ""
        if (overlay.scanning && (!root.view || root.view.bytes === undefined)) return "scanning"
        if (!root.view) return ""
        return root.view.name || ""
      }
      color: root.foreground
      opacity: 0.45
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideMiddle
    }
  }
}
