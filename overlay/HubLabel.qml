import QtQuick
import qs.Commons
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.menu.text

  readonly property var view: overlay ? overlay.currentView : null
  readonly property string name: {
    if (!overlay) return ""
    if (overlay.scanning && (!view || view.bytes === undefined)) return "Scanning…"
    if (!view) return overlay.scanRoot ? Format.bytes(0) : ""
    return view.name || ""
  }

  Column {
    anchors.centerIn: parent
    width: parent.width * 0.78
    spacing: Style.space(2)

    Text {
      width: parent.width
      text: root.view ? Format.bytes(root.view.bytes) : (overlay && overlay.scanning ? "…" : "0 B")
      color: root.foreground
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.display
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideRight
    }

    Text {
      width: parent.width
      text: root.name
      color: root.foreground
      opacity: 0.58
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.caption
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideMiddle
    }
  }
}
